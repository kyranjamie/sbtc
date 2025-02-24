use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use bitcoin::consensus::Encodable as _;
use bitcoin::hashes::Hash as _;
use bitcoin::Address;
use bitcoin::AddressType;
use bitcoin::Transaction;
use bitcoincore_rpc::RpcApi as _;
use blockstack_lib::chainstate::nakamoto::NakamotoBlock;
use blockstack_lib::chainstate::nakamoto::NakamotoBlockHeader;
use blockstack_lib::chainstate::stacks::StacksTransaction;
use blockstack_lib::chainstate::stacks::TransactionPayload;
use blockstack_lib::net::api::getcontractsrc::ContractSrcResponse;
use blockstack_lib::net::api::getpoxinfo::RPCPoxInfoData;
use blockstack_lib::net::api::getsortition::SortitionInfo;
use blockstack_lib::net::api::gettenureinfo::RPCGetTenureInfo;
use emily_client::apis::deposit_api;
use emily_client::apis::testing_api;
use emily_client::models::CreateDepositRequestBody;
use fake::Fake as _;
use fake::Faker;
use futures::StreamExt;
use mockito;
use rand::rngs::OsRng;
use rand::SeedableRng as _;
use reqwest;
use sbtc::testing::regtest;
use sbtc::testing::regtest::Recipient;
use secp256k1::Keypair;
use sha2::Digest as _;
use signer::network::in_memory2::WanNetwork;
use signer::stacks::api::TenureBlocks;
use signer::stacks::contracts::RotateKeysV1;
use signer::stacks::contracts::SmartContract;
use signer::storage::model::BitcoinTx;
use stacks_common::types::chainstate::BurnchainHeaderHash;
use stacks_common::types::chainstate::ConsensusHash;
use stacks_common::types::chainstate::SortitionId;
use stacks_common::types::chainstate::StacksBlockId;
use test_case::test_case;
use tokio_stream::wrappers::ReceiverStream;
use url::Url;

use signer::bitcoin::zmq::BitcoinCoreMessageStream;
use signer::block_observer::BlockObserver;
use signer::context::Context;
use signer::context::SignerEvent;
use signer::context::TxSignerEvent;
use signer::emily_client::EmilyClient;
use signer::error::Error;
use signer::keys;
use signer::keys::PublicKey;
use signer::keys::SignerScriptPubKey as _;
use signer::network;
use signer::network::in_memory::InMemoryNetwork;
use signer::stacks::api::AccountInfo;
use signer::stacks::api::MockStacksInteract;
use signer::stacks::api::SubmitTxResponse;
use signer::stacks::contracts::AsContractCall as _;
use signer::stacks::contracts::CompleteDepositV1;
use signer::stacks::contracts::SMART_CONTRACTS;
use signer::storage::model;
use signer::storage::model::EncryptedDkgShares;
use signer::storage::model::RotateKeysTransaction;
use signer::storage::postgres::PgStore;
use signer::storage::DbRead as _;
use signer::storage::DbWrite as _;
use signer::testing;
use signer::testing::context::TestContext;
use signer::testing::context::WrappedMock;
use signer::testing::context::*;
use signer::testing::storage::model::TestData;
use signer::testing::transaction_signer::TxSignerEventLoopHarness;
use signer::testing::wsts::SignerSet;
use signer::transaction_coordinator;
use signer::transaction_coordinator::TxCoordinatorEventLoop;
use signer::transaction_signer::TxSignerEventLoop;
use tokio::sync::broadcast::Sender;

use crate::complete_deposit::make_complete_deposit;
use crate::setup::backfill_bitcoin_blocks;
use crate::setup::TestSweepSetup;
use crate::utxo_construction::make_deposit_request;
use crate::zmq::BITCOIN_CORE_ZMQ_ENDPOINT;
use crate::DATABASE_NUM;

pub const GET_POX_INFO_JSON: &str =
    include_str!("../../tests/fixtures/stacksapi-get-pox-info-test-data.json");

async fn run_dkg<Rng, C>(
    ctx: &C,
    rng: &mut Rng,
    signer_set: &mut SignerSet,
) -> (keys::PublicKey, model::BitcoinBlockRef)
where
    C: Context + Send + Sync,
    Rng: rand::CryptoRng + rand::RngCore,
{
    let storage = ctx.get_storage_mut();

    let bitcoin_chain_tip = storage
        .get_bitcoin_canonical_chain_tip()
        .await
        .expect("storage error")
        .expect("no chain tip");

    let bitcoin_chain_tip_ref = storage
        .get_bitcoin_block(&bitcoin_chain_tip)
        .await
        .expect("storage failure")
        .expect("missing block")
        .into();

    let dkg_txid = testing::dummy::txid(&fake::Faker, rng);
    let (aggregate_key, all_dkg_shares) =
        signer_set.run_dkg(bitcoin_chain_tip, dkg_txid, rng).await;

    let encrypted_dkg_shares = all_dkg_shares.first().unwrap();
    signer_set
        .write_as_rotate_keys_tx(&storage, &bitcoin_chain_tip, encrypted_dkg_shares, rng)
        .await;

    let encrypted_dkg_shares = all_dkg_shares.first().unwrap();

    storage
        .write_encrypted_dkg_shares(encrypted_dkg_shares)
        .await
        .expect("failed to write encrypted shares");

    (aggregate_key, bitcoin_chain_tip_ref)
}

fn select_coordinator(
    bitcoin_chain_tip: &model::BitcoinBlockHash,
    signer_info: &[testing::wsts::SignerInfo],
) -> keys::PrivateKey {
    let mut hasher = sha2::Sha256::new();
    hasher.update(bitcoin_chain_tip.into_bytes());
    let digest = hasher.finalize();
    let index = usize::from_be_bytes(*digest.first_chunk().expect("unexpected digest size"));
    signer_info
        .get(index % signer_info.len())
        .expect("missing signer info")
        .signer_private_key
}

async fn push_utxo_donation<C>(ctx: &C, aggregate_key: &PublicKey, block_hash: &bitcoin::BlockHash)
where
    C: Context + Send + Sync,
{
    let tx = Transaction {
        version: bitcoin::transaction::Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![],
        output: vec![bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(1_337_000_000_000),
            script_pubkey: aggregate_key.signers_script_pubkey(),
        }],
    };

    let mut tx_bytes = Vec::new();
    tx.consensus_encode(&mut tx_bytes).unwrap();

    let tx = model::Transaction {
        txid: tx.compute_txid().to_byte_array(),
        tx: tx_bytes,
        tx_type: model::TransactionType::Donation,
        block_hash: *block_hash.as_byte_array(),
    };

    let bitcoin_transaction = model::BitcoinTxRef {
        txid: tx.txid.into(),
        block_hash: (*block_hash).into(),
    };

    ctx.get_storage_mut().write_transaction(&tx).await.unwrap();
    ctx.get_storage_mut()
        .write_bitcoin_transaction(&bitcoin_transaction)
        .await
        .unwrap();
}

async fn mock_reqwests_status_code_error(status_code: usize) -> reqwest::Error {
    let mut server: mockito::ServerGuard = mockito::Server::new_async().await;
    let _mock = server.mock("GET", "/").with_status(status_code).create();
    reqwest::get(server.url())
        .await
        .unwrap()
        .error_for_status()
        .expect_err("expected error")
}

fn mock_deploy_all_contracts(
    nonce: u64,
    broadcasted_transaction_tx: Sender<StacksTransaction>,
) -> Box<dyn FnOnce(&mut MockStacksInteract)> {
    Box::new(move |client: &mut MockStacksInteract| {
        // We expect the contract source to be fetched 5 times, once for
        // each contract. Each time it will return an error, since the
        // contracts are not deployed.
        client.expect_get_contract_source().returning(|_, _| {
            Box::pin(async {
                Err(Error::StacksNodeResponse(
                    mock_reqwests_status_code_error(404).await,
                ))
            })
        });
        // All the following functions, `estimate_fees`, `get_account` and
        // `submit_tx` are only called 5 times for the contracts to be
        // deployed and 1 for the deposit tx
        client
            .expect_estimate_fees()
            .returning(|_, _, _| Box::pin(async { Ok(100) }));

        client.expect_get_account().returning(move |_| {
            Box::pin(async move {
                Ok(AccountInfo {
                    balance: 1_000_000,
                    locked: 0,
                    unlock_height: 0,
                    nonce,
                })
            })
        });

        client.expect_submit_tx().returning(move |tx| {
            let tx = tx.clone();
            let txid = tx.txid();
            let broadcasted_transaction_tx = broadcasted_transaction_tx.clone();
            Box::pin(async move {
                broadcasted_transaction_tx
                    .send(tx)
                    .expect("Failed to send result");
                Ok(SubmitTxResponse::Acceptance(txid))
            })
        });
    })
}

fn mock_deploy_remaining_contracts_when_some_already_deployed(
    nonce: u64,
    broadcasted_transaction_tx: Sender<StacksTransaction>,
) -> Box<dyn FnOnce(&mut MockStacksInteract)> {
    Box::new(move |client: &mut MockStacksInteract| {
        // We expect the contract source to be fetched 5 times, once for
        // each contract. The first two times, it will return an
        // ContractSrcResponse, meaning that the contract was already
        // deployed.
        client
            .expect_get_contract_source()
            .times(2)
            .returning(|_, _| {
                Box::pin(async {
                    Ok(ContractSrcResponse {
                        source: String::new(),
                        publish_height: 1,
                        marf_proof: None,
                    })
                })
            });
        // The remaining 3 times, it will return an error, meaning that the
        // contracts were not deployed.
        client
            .expect_get_contract_source()
            .times(3)
            .returning(|_, _| {
                Box::pin(async {
                    Err(Error::StacksNodeResponse(
                        mock_reqwests_status_code_error(404).await,
                    ))
                })
            });

        // All the following functions, `estimate_fees`, `get_account` and
        // `submit_tx` are only called for the 3 contracts to be deployed
        // and once for the deposit tx
        client
            .expect_estimate_fees()
            .times(3)
            .returning(|_, _, _| Box::pin(async { Ok(100) }));

        client.expect_get_account().times(3).returning(move |_| {
            Box::pin(async move {
                Ok(AccountInfo {
                    balance: 1_000_000,
                    locked: 0,
                    unlock_height: 0,
                    nonce,
                })
            })
        });
        client.expect_submit_tx().times(3).returning(move |tx| {
            let tx = tx.clone();
            let txid = tx.txid();
            let broadcasted_transaction_tx = broadcasted_transaction_tx.clone();
            Box::pin(async move {
                broadcasted_transaction_tx
                    .send(tx)
                    .expect("Failed to send result");
                Ok(SubmitTxResponse::Acceptance(txid))
            })
        });
    })
}

fn mock_recover_and_deploy_all_contracts_after_failure(
    nonce: u64,
    broadcasted_transaction_tx: Sender<StacksTransaction>,
) -> Box<dyn FnOnce(&mut MockStacksInteract)> {
    Box::new(move |client: &mut MockStacksInteract| {
        // For the two contract we will return 404. Meaning that the
        // contract are not deployed yet.
        client
            .expect_get_contract_source()
            .times(2)
            .returning(|_, _| {
                Box::pin(async {
                    Err(Error::StacksNodeResponse(
                        mock_reqwests_status_code_error(404).await,
                    ))
                })
            });
        // While deploying the first contract, the estimate fees will be
        // called once successfully
        client
            .expect_estimate_fees()
            .once()
            .returning(|_, _, _| Box::pin(async { Ok(100) }));

        // In the process of deploying the second contract, the coordinator
        // will fail to estimate fees and it will abort the deployment It
        // will try again from scratch when It'll receive a second signal.
        client.expect_estimate_fees().times(1).returning(|_, _, _| {
            Box::pin(async {
                Err(Error::UnexpectedStacksResponse(
                    mock_reqwests_status_code_error(500).await,
                ))
            })
        });

        // The coordinator should try again from scratch. For the first
        // contract we will return the contract source as if it was already
        // deployed.
        client
            .expect_get_contract_source()
            .once()
            .returning(|_, _| {
                Box::pin(async {
                    Ok(ContractSrcResponse {
                        source: String::new(),
                        publish_height: 1,
                        marf_proof: None,
                    })
                })
            });

        // For the following 4 contracts we will return 404. So the
        // coordinator will try to deploy them.
        client
            .expect_get_contract_source()
            .times(4)
            .returning(|_, _| {
                Box::pin(async {
                    Err(Error::StacksNodeResponse(
                        mock_reqwests_status_code_error(404).await,
                    ))
                })
            });

        // Now for the remaining deploys we call estimate fees 4 more times.
        client
            .expect_estimate_fees()
            .times(4)
            .returning(|_, _, _| Box::pin(async { Ok(100) }));

        // `get_account` will be called 6 times, 2 for the first try to
        // deploy the contracts, 4 for the second try
        client.expect_get_account().times(6).returning(move |_| {
            Box::pin(async move {
                Ok(AccountInfo {
                    balance: 1_000_000,
                    locked: 0,
                    unlock_height: 0,
                    nonce,
                })
            })
        });
        // `submit_tx` will be called once for each contracts to be deployed
        client.expect_submit_tx().times(5).returning(move |tx| {
            let tx = tx.clone();
            let txid = tx.txid();
            let broadcasted_transaction_tx = broadcasted_transaction_tx.clone();
            Box::pin(async move {
                broadcasted_transaction_tx
                    .send(tx)
                    .expect("Failed to send result");
                Ok(SubmitTxResponse::Acceptance(txid))
            })
        });
    })
}

#[cfg_attr(not(feature = "integration-tests"), ignore)]
#[tokio::test]
async fn process_complete_deposit() {
    let db_num = DATABASE_NUM.fetch_add(1, Ordering::SeqCst);
    let db = testing::storage::new_test_database(db_num, true).await;
    let mut rng = rand::rngs::StdRng::seed_from_u64(51);
    let (rpc, faucet) = regtest::initialize_blockchain();
    let mut setup = TestSweepSetup::new_setup(&rpc, &faucet, 1_000_000, &mut rng);

    backfill_bitcoin_blocks(&db, rpc, &setup.sweep_block_hash).await;
    setup.store_deposit_tx(&db).await;
    setup.store_sweep_tx(&db).await;
    setup.store_dkg_shares(&db).await;
    setup.store_deposit_request(&db).await;
    setup.store_deposit_decisions(&db).await;
    // We need this to be able to store the sweep transaction, since
    // the `TestSweepSetup` includes a withdrawal by default. It's not used here,
    // though.
    setup.store_withdrawal_request(&db).await;
    // This will store the "broadcasted" sweep transactions.
    setup.store_sweep_transactions(&db).await;

    // Ensure a stacks tip exists
    let stacks_block = model::StacksBlock {
        block_hash: Faker.fake_with_rng(&mut OsRng),
        block_height: setup.sweep_block_height,
        parent_hash: Faker.fake_with_rng(&mut OsRng),
        bitcoin_anchor: setup.sweep_block_hash.into(),
    };
    db.write_stacks_block(&stacks_block).await.unwrap();

    let mut context = TestContext::builder()
        .with_storage(db.clone())
        .with_first_bitcoin_core_client()
        .with_mocked_stacks_client()
        .with_mocked_emily_client()
        .build();

    let nonce = 12;
    // Mock required stacks client functions
    context
        .with_stacks_client(|client| {
            client.expect_get_account().once().returning(move |_| {
                Box::pin(async move {
                    Ok(AccountInfo {
                        balance: 0,
                        locked: 0,
                        unlock_height: 0,
                        // The nonce is used to create the stacks tx
                        nonce,
                    })
                })
            });

            // Dummy value
            client
                .expect_estimate_fees()
                .once()
                .returning(move |_, _, _| Box::pin(async move { Ok(25505) }));
        })
        .await;

    let num_signers = 7;
    let signing_threshold = 5;
    let context_window = 10;

    let network = network::in_memory::InMemoryNetwork::new();
    let signer_info = testing::wsts::generate_signer_info(&mut rng, num_signers);

    let mut testing_signer_set =
        testing::wsts::SignerSet::new(&signer_info, signing_threshold, || network.connect());

    let (aggregate_key, bitcoin_chain_tip) =
        run_dkg(&context, &mut rng, &mut testing_signer_set).await;

    // Ensure we have a signers UTXO (as a donation, to not mess with the current
    // temporary `get_swept_deposit_requests` implementation)
    push_utxo_donation(&context, &aggregate_key, &setup.sweep_block_hash).await;

    assert_eq!(
        context
            .get_storage()
            .get_swept_deposit_requests(&bitcoin_chain_tip.block_hash, context_window)
            .await
            .expect("failed to get swept deposits")
            .len(),
        1
    );

    let (broadcasted_transaction_tx, _broadcasted_transaction_rxeiver) =
        tokio::sync::broadcast::channel(1);

    // This task logs all transactions broadcasted by the coordinator.
    let mut wait_for_transaction_rx = broadcasted_transaction_tx.subscribe();
    let wait_for_transaction_task =
        tokio::spawn(async move { wait_for_transaction_rx.recv().await });

    // Setup the stacks client mock to broadcast the transaction to our channel.
    context
        .with_stacks_client(|client| {
            client.expect_submit_tx().once().returning(move |tx| {
                let tx = tx.clone();
                let txid = tx.txid();
                let broadcasted_transaction_tx = broadcasted_transaction_tx.clone();
                Box::pin(async move {
                    broadcasted_transaction_tx
                        .send(tx)
                        .expect("Failed to send result");
                    Ok(SubmitTxResponse::Acceptance(txid))
                })
            });

            client
                .expect_get_current_signers_aggregate_key()
                .returning(move |_| Box::pin(std::future::ready(Ok(Some(aggregate_key)))));
        })
        .await;

    // Get the private key of the coordinator of the signer set.
    let private_key = select_coordinator(&setup.sweep_block_hash.into(), &signer_info);

    // Bootstrap the tx coordinator event loop
    let tx_coordinator = transaction_coordinator::TxCoordinatorEventLoop {
        context: context.clone(),
        network: network.connect(),
        private_key,
        context_window,
        threshold: signing_threshold as u16,
        signing_round_max_duration: Duration::from_secs(10),
        dkg_max_duration: Duration::from_secs(10),
        sbtc_contracts_deployed: true,
        is_epoch3: true,
    };
    let tx_coordinator_handle = tokio::spawn(async move { tx_coordinator.run().await });

    // TODO: here signers use all the same storage, should we use separate ones?
    let event_loop_handles: Vec<_> = signer_info
        .clone()
        .into_iter()
        .map(|signer_info| {
            let event_loop_harness = TxSignerEventLoopHarness::create(
                context.clone(),
                network.connect(),
                context_window,
                signer_info.signer_private_key,
                signing_threshold,
                rng.clone(),
            );

            event_loop_harness.start()
        })
        .collect();

    // Yield to get signers ready
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Wake coordinator up
    context
        .signal(SignerEvent::TxSigner(TxSignerEvent::NewRequestsHandled).into())
        .expect("failed to signal");

    // Await the `wait_for_tx_task` to receive the first transaction broadcasted.
    let broadcasted_tx = tokio::time::timeout(Duration::from_secs(10), wait_for_transaction_task)
        .await
        .unwrap()
        .expect("failed to receive message")
        .expect("no message received");

    // Stop event loops
    tx_coordinator_handle.abort();
    event_loop_handles.iter().for_each(|h| h.abort());

    broadcasted_tx.verify().unwrap();

    assert_eq!(broadcasted_tx.get_origin_nonce(), nonce);

    let (complete_deposit, _) = make_complete_deposit(&setup);
    let TransactionPayload::ContractCall(contract_call) = broadcasted_tx.payload else {
        panic!("unexpected tx payload")
    };
    assert_eq!(
        contract_call.contract_name.to_string(),
        CompleteDepositV1::CONTRACT_NAME
    );
    assert_eq!(
        contract_call.function_name.to_string(),
        CompleteDepositV1::FUNCTION_NAME
    );
    assert_eq!(
        contract_call.function_args,
        complete_deposit.as_contract_args()
    );

    testing::storage::drop_db(db).await;
}

#[ignore = "These tests take ~10 seconds per test case to run"]
#[test_case(&SMART_CONTRACTS, mock_deploy_all_contracts; "deploy-all-contracts")]
#[test_case(&SMART_CONTRACTS[2..], mock_deploy_remaining_contracts_when_some_already_deployed; "deploy-remaining-contracts-when-some-already-deployed")]
#[test_case(&SMART_CONTRACTS, mock_recover_and_deploy_all_contracts_after_failure; "recover-and-deploy-all-contracts-after-failure")]
#[tokio::test]
async fn deploy_smart_contracts_coordinator<F>(
    smart_contracts: &[SmartContract],
    stacks_client_mock: F,
) where
    F: FnOnce(u64, Sender<StacksTransaction>) -> Box<dyn FnOnce(&mut MockStacksInteract)>,
{
    signer::logging::setup_logging("info", false);
    let db_num = DATABASE_NUM.fetch_add(1, Ordering::SeqCst);
    let db = testing::storage::new_test_database(db_num, true).await;
    let mut rng = rand::rngs::StdRng::seed_from_u64(51);

    let num_messages = smart_contracts.len();

    let bitcoin_block: model::BitcoinBlock = Faker.fake_with_rng(&mut rng);
    db.write_bitcoin_block(&bitcoin_block).await.unwrap();

    // Ensure a stacks tip exists
    let mut stacks_block: model::StacksBlock = Faker.fake_with_rng(&mut rng);
    stacks_block.bitcoin_anchor = bitcoin_block.block_hash;
    db.write_stacks_block(&stacks_block).await.unwrap();

    let mut context = TestContext::builder()
        .with_storage(db.clone())
        .with_mocked_clients()
        .build();

    let nonce = 12;

    let num_signers = 7;
    let signing_threshold = 5;
    let context_window = 10;

    let network = network::in_memory::InMemoryNetwork::new();
    let signer_info: Vec<testing::wsts::SignerInfo> =
        testing::wsts::generate_signer_info(&mut rng, num_signers);

    let mut testing_signer_set =
        testing::wsts::SignerSet::new(&signer_info, signing_threshold, || network.connect());

    let (_, bitcoin_chain_tip) = run_dkg(&context, &mut rng, &mut testing_signer_set).await;

    // Mock the stacks client for the TxSigners that will validate
    // the contract source before signing the transaction.
    context
        .with_stacks_client(|client| {
            client.expect_get_contract_source().returning(|_, _| {
                Box::pin(async {
                    Err(Error::StacksNodeResponse(
                        mock_reqwests_status_code_error(404).await,
                    ))
                })
            });
        })
        .await;

    let (broadcasted_transaction_tx, _broadcasted_transaction_rxeiver) =
        tokio::sync::broadcast::channel(1);

    // This task logs all transactions broadcasted by the coordinator.
    let mut wait_for_transaction_rx = broadcasted_transaction_tx.subscribe();
    let wait_for_transaction_task = tokio::spawn(async move {
        let mut results = Vec::with_capacity(num_messages);
        for _ in 0..num_messages {
            results.push(wait_for_transaction_rx.recv().await);
        }
        results
    });

    // Create a new context for the tx coordinator. This is necessary because
    // the tx signers and the tx coordinator will use a different stacks mock client.
    // Note that cloning the context will `Arc::clone` the stacks client, so we are
    // instantiating a new one instead
    let mut tx_coordinator_context = TestContext::new(
        context.config().clone(),
        context.storage.clone(),
        context.bitcoin_client.clone(),
        WrappedMock::default(),
        context.emily_client.clone(),
    );
    // Mock the stacks client for the TxCoordinator that will deploy the contracts.
    tx_coordinator_context
        .with_stacks_client(stacks_client_mock(nonce, broadcasted_transaction_tx))
        .await;

    // Get the private key of the coordinator of the signer set.
    let private_key = select_coordinator(&bitcoin_chain_tip.block_hash, &signer_info);

    // Bootstrap the tx coordinator event loop
    let tx_coordinator = transaction_coordinator::TxCoordinatorEventLoop {
        context: tx_coordinator_context.clone(),
        network: network.connect(),
        private_key,
        context_window,
        threshold: signing_threshold as u16,
        signing_round_max_duration: Duration::from_secs(10),
        dkg_max_duration: Duration::from_secs(10),
        sbtc_contracts_deployed: false,
        is_epoch3: true,
    };
    let tx_coordinator_handle = tokio::spawn(async move { tx_coordinator.run().await });

    // TODO: here signers use all the same storage, should we use separate ones?
    let event_loop_handles: Vec<_> = signer_info
        .clone()
        .into_iter()
        .map(|signer_info| {
            let event_loop_harness = TxSignerEventLoopHarness::create(
                context.clone(),
                network.connect(),
                context_window,
                signer_info.signer_private_key,
                signing_threshold,
                rng.clone(),
            );

            event_loop_harness.start()
        })
        .collect();

    // Yield to get signers ready
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Wake coordinator up
    tx_coordinator_context
        .signal(SignerEvent::TxSigner(TxSignerEvent::NewRequestsHandled).into())
        .expect("failed to signal");
    // Send a second signal to pick up the request after an error
    // used in the recover-and-deploy-all-contracts-after-failure test case
    tx_coordinator_context
        .signal(SignerEvent::TxSigner(TxSignerEvent::NewRequestsHandled).into())
        .expect("failed to signal");

    let broadcasted_txs = tokio::time::timeout(Duration::from_secs(10), wait_for_transaction_task)
        .await
        .unwrap()
        .expect("failed to receive message");

    assert_eq!(broadcasted_txs.len(), smart_contracts.len());

    // Check that the contracts were deployed
    for (deployed, broadcasted_tx) in smart_contracts.iter().zip(broadcasted_txs) {
        let broadcasted_tx = broadcasted_tx.expect("expected a tx");
        // Await the `wait_for_tx_task` to receive the first transaction broadcasted.
        broadcasted_tx.verify().unwrap();

        assert_eq!(broadcasted_tx.get_origin_nonce(), nonce);
        let TransactionPayload::SmartContract(contract, _) = broadcasted_tx.payload else {
            panic!("unexpected tx payload")
        };
        assert_eq!(contract.name.as_str(), deployed.contract_name());
        assert_eq!(&contract.code_body.to_string(), deployed.contract_body());
    }

    // Stop event loops
    tx_coordinator_handle.abort();
    event_loop_handles.iter().for_each(|h| h.abort());

    testing::storage::drop_db(db).await;
}

/// The [`TxCoordinatorEventLoop::get_signer_set_and_aggregate_key`]
/// function is supposed to fetch the "current" signing set and the
/// aggregate key to use for bitcoin transactions. It attempts to get the
/// latest rotate-keys contract call transaction confirmed on the canonical
/// Stacks blockchain and falls back to the DKG shares table if no such
/// transaction can be found.
///
/// This tests that we prefer rotate keys transactions if it's available
/// but will use the DKG shares behavior is indeed the case.
#[cfg_attr(not(feature = "integration-tests"), ignore)]
#[tokio::test]
async fn get_signer_public_keys_and_aggregate_key_falls_back() {
    let db_num = DATABASE_NUM.fetch_add(1, Ordering::SeqCst);
    let db = testing::storage::new_test_database(db_num, true).await;

    let mut rng = rand::rngs::StdRng::seed_from_u64(51);

    let ctx = TestContext::builder()
        .with_storage(db.clone())
        .with_mocked_clients()
        .build();

    let network = InMemoryNetwork::new();

    let coord = TxCoordinatorEventLoop {
        network: network.connect(),
        context: ctx.clone(),
        context_window: 10000,
        private_key: ctx.config().signer.private_key,
        signing_round_max_duration: Duration::from_secs(10),
        threshold: 2,
        dkg_max_duration: Duration::from_secs(10),
        sbtc_contracts_deployed: true, // Skip contract deployment
        is_epoch3: true,
    };

    // We need stacks blocks for the rotate-keys transactions.
    let test_params = testing::storage::model::Params {
        num_bitcoin_blocks: 10,
        num_stacks_blocks_per_bitcoin_block: 1,
        num_deposit_requests_per_block: 0,
        num_withdraw_requests_per_block: 0,
        num_signers_per_request: 0,
    };
    let test_data = TestData::generate(&mut rng, &[], &test_params);
    test_data.write_to(&db).await;

    // We always need the chain tip.
    let chain_tip = db.get_bitcoin_canonical_chain_tip().await.unwrap().unwrap();

    // We have no rows in the DKG shares table and no rotate-keys
    // transactions, so there should be no aggregate key, since that only
    // happens after DKG, but we should always know the current signer set.
    let (maybe_aggregate_key, signer_set) = coord
        .get_signer_set_and_aggregate_key(&chain_tip)
        .await
        .unwrap();
    assert!(maybe_aggregate_key.is_none());
    assert!(!signer_set.is_empty());

    // Alright, lets write some DKG shares into the database. When we do
    // that the signer set should be considered whatever the signer set is
    // from our DKG shares.
    let shares: EncryptedDkgShares = Faker.fake_with_rng(&mut rng);
    db.write_encrypted_dkg_shares(&shares).await.unwrap();

    let (aggregate_key, signer_set) = coord
        .get_signer_set_and_aggregate_key(&chain_tip)
        .await
        .unwrap();

    let shares_signer_set: BTreeSet<PublicKey> =
        shares.signer_set_public_keys.iter().copied().collect();

    assert_eq!(shares.aggregate_key, aggregate_key.unwrap());
    assert_eq!(shares_signer_set, signer_set);

    // Okay not we write a rotate-keys transaction into the database. To do
    // that we need the stacks chain tip, and a something in 3 different
    // tables...
    let stacks_chain_tip = db.get_stacks_chain_tip(&chain_tip).await.unwrap().unwrap();

    let rotate_keys: RotateKeysTransaction = Faker.fake_with_rng(&mut rng);
    let transaction = model::Transaction {
        txid: rotate_keys.txid.into_bytes(),
        tx: Vec::new(),
        tx_type: model::TransactionType::RotateKeys,
        block_hash: stacks_chain_tip.block_hash.into_bytes(),
    };
    let tx = model::StacksTransaction {
        txid: rotate_keys.txid,
        block_hash: stacks_chain_tip.block_hash,
    };

    db.write_transaction(&transaction).await.unwrap();
    db.write_stacks_transaction(&tx).await.unwrap();
    db.write_rotate_keys_transaction(&rotate_keys)
        .await
        .unwrap();

    // Alright, now that we have a rotate-keys transaction, we can check if
    // it is preferred over the DKG shares table.
    let (aggregate_key, signer_set) = coord
        .get_signer_set_and_aggregate_key(&chain_tip)
        .await
        .unwrap();

    let rotate_keys_signer_set: BTreeSet<PublicKey> =
        rotate_keys.signer_set.iter().copied().collect();

    assert_eq!(rotate_keys.aggregate_key, aggregate_key.unwrap());
    assert_eq!(rotate_keys_signer_set, signer_set);

    testing::storage::drop_db(db).await;
}

/// Test that we run DKG if the coordinator notices that DKG has not been
/// run yet.
///
/// This test proceeds by doing the following:
/// 1. Create a database, an associated context, and a Keypair for each of
///    the signers in the signing set.
/// 2. Populate each database with the same data, so that they have the
///    same view of the canonical bitcoin blockchain. This ensures that
///    they participate in DKG.
/// 3. Check that there are no DKG shares in the database.
/// 4. Start the [`TxCoordinatorEventLoop`] and [`TxSignerEventLoop`]
///    processes for each signer.
/// 5. Once they are all running, signal that DKG should be run. We signal
///    them all because we do not know which one is the coordinator.
/// 6. Check that we have exactly one row in the `dkg_shares` table.
/// 7. Check that they all have the same aggregate key in the `dkg_shares`
///    table.
/// 8. Check that the coordinator broadcast a rotate key tx
///
/// Some of the preconditions for this test to run successfully includes
/// having bootstrap public keys that align with the [`Keypair`] returned
/// from the [`testing::wallet::regtest_bootstrap_wallet`] function.
#[cfg_attr(not(feature = "integration-tests"), ignore)]
#[tokio::test]
async fn run_dkg_from_scratch() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(51);
    let (signer_wallet, signer_key_pairs): (_, [Keypair; 3]) =
        testing::wallet::regtest_bootstrap_wallet();

    // We need to populate our databases, so let's generate some data.
    let test_params = testing::storage::model::Params {
        num_bitcoin_blocks: 10,
        num_stacks_blocks_per_bitcoin_block: 1,
        num_deposit_requests_per_block: 0,
        num_withdraw_requests_per_block: 0,
        num_signers_per_request: 0,
    };
    let test_data = TestData::generate(&mut rng, &[], &test_params);

    let (broadcast_stacks_tx, _rx) = tokio::sync::broadcast::channel(1);

    let mut stacks_tx_receiver = broadcast_stacks_tx.subscribe();
    let stacks_tx_receiver_task = tokio::spawn(async move { stacks_tx_receiver.recv().await });

    let iter: Vec<(Keypair, TestData)> = signer_key_pairs
        .iter()
        .copied()
        .zip(std::iter::repeat_with(|| test_data.clone()))
        .collect();

    // 1. Create a database, an associated context, and a Keypair for each of
    //    the signers in the signing set.
    let signers: Vec<(_, PgStore, Keypair)> = futures::stream::iter(iter)
        .then(|(kp, data)| {
            let broadcast_stacks_tx = broadcast_stacks_tx.clone();
            async move {
                let db_num = DATABASE_NUM.fetch_add(1, Ordering::SeqCst);
                let db = testing::storage::new_test_database(db_num, true).await;
                let mut ctx = TestContext::builder()
                    .with_storage(db.clone())
                    .with_mocked_clients()
                    .build();

                ctx.with_stacks_client(|client| {
                    client
                        .expect_estimate_fees()
                        .returning(|_, _, _| Box::pin(async { Ok(123000) }));

                    client.expect_get_account().returning(|_| {
                        Box::pin(async {
                            Ok(AccountInfo {
                                balance: 1_000_000,
                                locked: 0,
                                unlock_height: 0,
                                nonce: 1,
                            })
                        })
                    });

                    client.expect_submit_tx().returning(move |tx| {
                        let tx = tx.clone();
                        let txid = tx.txid();
                        let broadcast_stacks_tx = broadcast_stacks_tx.clone();
                        Box::pin(async move {
                            broadcast_stacks_tx.send(tx).expect("Failed to send result");
                            Ok(SubmitTxResponse::Acceptance(txid))
                        })
                    });

                    client
                        .expect_get_current_signers_aggregate_key()
                        .returning(move |_| {
                            // We want to test the tx submission
                            Box::pin(std::future::ready(Ok(None)))
                        });
                })
                .await;

                // 2. Populate each database with the same data, so that they
                //    have the same view of the canonical bitcoin blockchain.
                //    This ensures that they participate in DKG.
                data.write_to(&db).await;

                (ctx, db, kp)
            }
        })
        .collect::<Vec<_>>()
        .await;

    let network = InMemoryNetwork::new();

    // 3. Check that there are no DKG shares in the database.
    for (_, db, _) in signers.iter() {
        let some_shares = db.get_latest_encrypted_dkg_shares().await.unwrap();
        assert!(some_shares.is_none());
    }

    // 4. Start the [`TxCoordinatorEventLoop`] and [`TxSignerEventLoop`]
    //    processes for each signer.
    let tx_coordinator_processes = signers.iter().map(|(ctx, _, kp)| TxCoordinatorEventLoop {
        network: network.connect(),
        context: ctx.clone(),
        context_window: 10000,
        private_key: kp.secret_key().into(),
        signing_round_max_duration: Duration::from_secs(10),
        threshold: ctx.config().signer.bootstrap_signatures_required,
        dkg_max_duration: Duration::from_secs(10),
        sbtc_contracts_deployed: true, // Skip contract deployment
        is_epoch3: true,
    });

    let tx_signer_processes = signers.iter().map(|(context, _, kp)| TxSignerEventLoop {
        network: network.connect(),
        threshold: context.config().signer.bootstrap_signatures_required as u32,
        context: context.clone(),
        context_window: 10000,
        blocklist_checker: Some(()),
        wsts_state_machines: HashMap::new(),
        signer_private_key: kp.secret_key().into(),
        rng: rand::rngs::OsRng,
    });

    // We only proceed with the test after all processes have started, and
    // we use this counter to notify us when that happens.
    let start_count = Arc::new(AtomicU8::new(0));

    tx_coordinator_processes.for_each(|ev| {
        let counter = start_count.clone();
        tokio::spawn(async move {
            counter.fetch_add(1, Ordering::Relaxed);
            ev.run().await
        });
    });

    tx_signer_processes.for_each(|ev| {
        let counter = start_count.clone();
        tokio::spawn(async move {
            counter.fetch_add(1, Ordering::Relaxed);
            ev.run().await
        });
    });

    while start_count.load(Ordering::SeqCst) < 6 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // 5. Once they are all running, signal that DKG should be run. We
    //    signal them all because we do not know which one is the
    //    coordinator.
    signers.iter().for_each(|(ctx, _, _)| {
        ctx.get_signal_sender()
            .send(TxSignerEvent::NewRequestsHandled.into())
            .unwrap();
    });

    // Await the `stacks_tx_receiver_task` to receive the first transaction broadcasted.
    let broadcast_stacks_txs =
        tokio::time::timeout(Duration::from_secs(10), stacks_tx_receiver_task)
            .await
            .unwrap()
            .expect("failed to receive message")
            .expect("no message received");

    let mut aggregate_keys = BTreeSet::new();

    for (_, db, _) in signers.iter() {
        let mut aggregate_key =
            sqlx::query_as::<_, (PublicKey,)>("SELECT aggregate_key FROM sbtc_signer.dkg_shares")
                .fetch_all(db.pool())
                .await
                .unwrap();

        // 6. Check that we have exactly one row in the `dkg_shares` table.
        assert_eq!(aggregate_key.len(), 1);

        // An additional sanity check that the query in
        // get_last_encrypted_dkg_shares gets the right thing (which is the
        // only thing in this case.)
        let key = aggregate_key.pop().unwrap().0;
        let shares = db.get_latest_encrypted_dkg_shares().await.unwrap().unwrap();
        assert_eq!(shares.aggregate_key, key);
        aggregate_keys.insert(key);
    }

    // 7. Check that they all have the same aggregate key in the
    //    `dkg_shares` table.
    assert_eq!(aggregate_keys.len(), 1);

    // 8. Check that the coordinator broadcast a rotate key tx
    broadcast_stacks_txs.verify().unwrap();

    let TransactionPayload::ContractCall(contract_call) = broadcast_stacks_txs.payload else {
        panic!("unexpected tx payload")
    };
    assert_eq!(
        contract_call.contract_name.to_string(),
        RotateKeysV1::CONTRACT_NAME
    );
    assert_eq!(
        contract_call.function_name.to_string(),
        RotateKeysV1::FUNCTION_NAME
    );
    let rotate_keys = RotateKeysV1::new(
        &signer_wallet,
        signers.first().unwrap().0.config().signer.deployer,
        aggregate_keys.iter().next().unwrap(),
    );
    assert_eq!(contract_call.function_args, rotate_keys.as_contract_args());

    for (ctx, db, _) in signers {
        ctx.get_termination_handle().signal_shutdown();
        testing::storage::drop_db(db).await;
    }
}

/// Test that three signers can successfully sign and broadcast a bitcoin
/// transaction.
///
/// The test setup is as follows:
/// 1. There are three "signers" contexts. Each context points to its own
///    real postgres database, and they have their own private key. Each
///    database is populated with the same data.
/// 2. Each context is given to a block observer, a tx signer, and a tx
///    coordinator, where these event loops are spawned as separate tasks.
/// 3. The signers communicate with our in-memory network struct.
/// 4. A real Emily server is running in the background.
/// 5. A real bitcoin-core node is running in the background.
/// 6. Stacks-core is mocked.
///
/// After the setup, the signers observe a bitcoin block and update their
/// databases. The coordinator then constructs a bitcoin transaction and
/// gets it signed. After it is signed the coordinator broadcasts it to
/// bitcoin-core.
///
/// To start the test environment do:
/// ```bash
/// make integration-env-up-ci
/// ```
///
/// then, once everything is up and running, run the test.
///
/// We can activate this test once we fixed our in-memory network thing to
/// not duplicate messages, like we do in prod.
// #[cfg_attr(not(feature = "integration-tests"), ignore)]
#[ignore]
#[tokio::test]
async fn sign_bitcoin_transaction() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(51);
    let (_, signer_key_pairs): (_, [Keypair; 3]) = testing::wallet::regtest_bootstrap_wallet();
    let (rpc, faucet) = regtest::initialize_blockchain();
    signer::logging::setup_logging("info", false);

    // We need to populate our databases, so let's fetch the data.
    let emily_client =
        EmilyClient::try_from(&Url::parse("http://localhost:3031").unwrap()).unwrap();

    testing_api::wipe_databases(emily_client.config())
        .await
        .unwrap();

    let network = WanNetwork::default();

    let chain_tip_info = rpc.get_chain_tips().unwrap().pop().unwrap();

    // 1. Create a database, an associated context, and a Keypair for each of
    //    the signers in the signing set.
    let mut signers = Vec::new();
    for kp in signer_key_pairs.iter() {
        let db_num = DATABASE_NUM.fetch_add(1, Ordering::SeqCst);
        let db = testing::storage::new_test_database(db_num, true).await;
        let ctx = TestContext::builder()
            .with_storage(db.clone())
            .with_first_bitcoin_core_client()
            .with_emily_client(emily_client.clone())
            .with_mocked_stacks_client()
            .build();

        // 2. Populate each database with the same data, so that they
        //    have the same view of the canonical bitcoin blockchain.
        //    This ensures that they participate in DKG.
        backfill_bitcoin_blocks(&db, rpc, &chain_tip_info.hash).await;

        let network = network.connect().await;

        signers.push((ctx, db, kp, network));
    }

    let (broadcast_stacks_tx, _rx) = tokio::sync::broadcast::channel(1);

    let mut stacks_tx_receiver = broadcast_stacks_tx.subscribe();
    let stacks_tx_receiver_task = tokio::spawn(async move { stacks_tx_receiver.recv().await });

    for (ctx, _, _, _) in signers.iter_mut() {
        let broadcast_stacks_tx = broadcast_stacks_tx.clone();
        ctx.with_stacks_client(|client| {
            client.expect_get_tenure_info().returning(move || {
                let response = Ok(RPCGetTenureInfo {
                    consensus_hash: ConsensusHash([0; 20]),
                    tenure_start_block_id: StacksBlockId([0; 32]),
                    parent_consensus_hash: ConsensusHash([0; 20]),
                    parent_tenure_start_block_id: StacksBlockId::first_mined(),
                    tip_block_id: StacksBlockId([0; 32]),
                    tip_height: 0,
                    reward_cycle: 0,
                });
                Box::pin(std::future::ready(response))
            });

            client.expect_get_block().returning(|_| {
                let response = Ok(NakamotoBlock {
                    header: NakamotoBlockHeader::empty(),
                    txs: vec![],
                });
                Box::pin(std::future::ready(response))
            });

            let chain_tip = model::BitcoinBlockHash::from(chain_tip_info.hash);
            client.expect_get_tenure().returning(move |_| {
                let mut tenure = TenureBlocks::nearly_empty().unwrap();
                tenure.anchor_block_hash = chain_tip;
                Box::pin(std::future::ready(Ok(tenure)))
            });

            client.expect_get_pox_info().returning(|| {
                let response = serde_json::from_str::<RPCPoxInfoData>(GET_POX_INFO_JSON)
                    .map_err(Error::JsonSerialize);
                Box::pin(std::future::ready(response))
            });

            client
                .expect_estimate_fees()
                .returning(|_, _, _| Box::pin(std::future::ready(Ok(25))));

            // The coordinator will try to further process the deposit to submit
            // the stacks tx, but we are not interested (for the current test iteration).
            client.expect_get_account().returning(|_| {
                let response = Ok(AccountInfo {
                    balance: 0,
                    locked: 0,
                    unlock_height: 0,
                    // this is the only part used to create the stacks transaction.
                    nonce: 12,
                });
                Box::pin(std::future::ready(response))
            });
            client.expect_get_sortition_info().returning(move |_| {
                let response = Ok(SortitionInfo {
                    burn_block_hash: BurnchainHeaderHash::from(chain_tip),
                    burn_block_height: chain_tip_info.height,
                    burn_header_timestamp: 0,
                    sortition_id: SortitionId([0; 32]),
                    parent_sortition_id: SortitionId([0; 32]),
                    consensus_hash: ConsensusHash([0; 20]),
                    was_sortition: true,
                    miner_pk_hash160: None,
                    stacks_parent_ch: None,
                    last_sortition_ch: None,
                    committed_block_hash: None,
                });
                Box::pin(std::future::ready(response))
            });

            // Only the client that corresponds to the coordinator will
            // submit a transaction so we don't make explicit the
            // expectation here.
            client.expect_submit_tx().returning(move |tx| {
                let tx = tx.clone();
                let txid = tx.txid();
                let broadcast_stacks_tx = broadcast_stacks_tx.clone();
                Box::pin(async move {
                    broadcast_stacks_tx.send(tx).unwrap();
                    Ok(SubmitTxResponse::Acceptance(txid))
                })
            });
        })
        .await;
    }

    // 4. Start the [`TxCoordinatorEventLoop`] and [`TxSignerEventLoop`]
    //    processes for each signer.

    // We only proceed with the test after all processes have started, and
    // we use this counter to notify us when that happens.
    let start_count = Arc::new(AtomicU8::new(0));

    for (ctx, _, kp, network) in signers.iter() {
        let ev = TxCoordinatorEventLoop {
            network: network.spawn(),
            context: ctx.clone(),
            context_window: 10000,
            private_key: kp.secret_key().into(),
            signing_round_max_duration: Duration::from_secs(10),
            threshold: ctx.config().signer.bootstrap_signatures_required,
            dkg_max_duration: Duration::from_secs(10),
            sbtc_contracts_deployed: true,
            is_epoch3: true,
        };
        let counter = start_count.clone();
        tokio::spawn(async move {
            counter.fetch_add(1, Ordering::Relaxed);
            ev.run().await
        });
    }

    for (context, _, kp, network) in signers.iter() {
        let ev = TxSignerEventLoop {
            network: network.spawn(),
            threshold: context.config().signer.bootstrap_signatures_required as u32,
            context: context.clone(),
            context_window: 10000,
            blocklist_checker: Some(()),
            wsts_state_machines: HashMap::new(),
            signer_private_key: kp.secret_key().into(),
            rng: rand::rngs::OsRng,
        };
        let counter = start_count.clone();
        tokio::spawn(async move {
            counter.fetch_add(1, Ordering::Relaxed);
            ev.run().await
        });
    }

    for (ctx, _, _, _) in signers.iter() {
        let counter = start_count.clone();

        let zmq_stream =
            BitcoinCoreMessageStream::new_from_endpoint(BITCOIN_CORE_ZMQ_ENDPOINT, &["hashblock"])
                .await
                .unwrap();
        let (sender, receiver) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            let mut stream = zmq_stream.to_block_hash_stream();
            while let Some(block) = stream.next().await {
                sender.send(block).await.unwrap();
            }
        });

        let block_observer = BlockObserver {
            context: ctx.clone(),
            stacks_client: ctx.stacks_client.clone(),
            emily_client: ctx.emily_client.clone(),
            bitcoin_blocks: ReceiverStream::new(receiver),
            horizon: 10,
        };

        tokio::spawn(async move {
            counter.fetch_add(1, Ordering::Relaxed);
            block_observer.run().await
        });
    }

    while start_count.load(Ordering::SeqCst) < 9 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    faucet.generate_blocks(1);

    // 5. Once they are all running, signal that DKG should be run. We
    //    signal them all because we do not know which one is the
    //    coordinator.

    tokio::time::sleep(Duration::from_secs(3)).await;

    let mut shares: EncryptedDkgShares = Faker.fake_with_rng(&mut rng);
    for (_, db, _, _) in signers.iter() {
        shares = db.get_latest_encrypted_dkg_shares().await.unwrap().unwrap();
    }

    let script_pub_key = shares.aggregate_key.signers_script_pubkey();
    let network = bitcoin::Network::Regtest;
    let address = Address::from_script(&script_pub_key, network).unwrap();

    faucet.send_to(100_000, &address);

    let depositor = Recipient::new(AddressType::P2tr);

    // Start off with some initial UTXOs to work with.

    faucet.send_to(50_000_000, &depositor.address);
    faucet.generate_blocks(1);

    // Now lets make a deposit transaction and submit it
    let depositor_utxo = depositor.get_utxos(rpc, None).pop().unwrap();

    let amount = 2_500_000;
    let signers_public_key = shares.aggregate_key.into();
    let (deposit_tx, deposit_request, _) =
        make_deposit_request(&depositor, amount, depositor_utxo, signers_public_key);
    rpc.send_raw_transaction(&deposit_tx).unwrap();

    assert_eq!(deposit_tx.compute_txid(), deposit_request.outpoint.txid);

    let body = CreateDepositRequestBody {
        bitcoin_tx_output_index: deposit_request.outpoint.vout,
        bitcoin_txid: deposit_request.outpoint.txid.to_string(),
        deposit_script: deposit_request.deposit_script.to_hex_string(),
        reclaim_script: deposit_request.reclaim_script.to_hex_string(),
    };
    let _ = deposit_api::create_deposit(emily_client.config(), body)
        .await
        .unwrap();

    faucet.generate_blocks(1);

    tokio::time::sleep(Duration::from_secs(5)).await;

    let (ctx, _, _, _) = signers.first().unwrap();
    let mut txids = ctx.bitcoin_client.inner_client().get_raw_mempool().unwrap();
    assert_eq!(txids.len(), 1);

    let block_hash = faucet.generate_blocks(1).pop().unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Let's check that the contract call transaction was made.
    let broadcast_stacks_txs =
        tokio::time::timeout(Duration::from_secs(10), stacks_tx_receiver_task)
            .await
            .unwrap()
            .expect("failed to receive message")
            .unwrap();

    let TransactionPayload::ContractCall(contract_call) = broadcast_stacks_txs.payload else {
        panic!("expected a contract call, got something else");
    };

    assert_eq!(
        contract_call.contract_name.as_str(),
        CompleteDepositV1::CONTRACT_NAME
    );
    assert_eq!(
        contract_call.function_name.as_str(),
        CompleteDepositV1::FUNCTION_NAME
    );

    // Now lets check the bitcoin transaction
    let txid = txids.pop().unwrap();
    let tx_info = ctx
        .bitcoin_client
        .get_tx_info(&txid, &block_hash)
        .unwrap()
        .unwrap();
    let actual_script_pub_key = tx_info.vin[0].prevout.script_pub_key.script.as_bytes();

    assert_eq!(actual_script_pub_key, script_pub_key.as_bytes());
    assert_eq!(&tx_info.tx.output[0].script_pubkey, &script_pub_key);

    let tx = sqlx::query_scalar::<_, BitcoinTx>(
        r#"
        SELECT tx
        FROM sbtc_signer.transactions
        WHERE txid = $1
        "#,
    )
    .bind(txid.to_byte_array())
    .fetch_one(ctx.storage.pool())
    .await
    .unwrap();

    let script = tx.output[0].script_pubkey.clone().into();
    for (_, db, _, _) in signers {
        assert!(db.is_signer_script_pub_key(&script).await.unwrap());
        testing::storage::drop_db(db).await;
    }
}
