use bitcoin::hashes::Hash as _;
use bitcoin::AddressType;
use bitcoin::OutPoint;
use bitcoincore_rpc::Client;
use bitcoincore_rpc::RpcApi as _;
use blockstack_lib::types::chainstate::StacksAddress;
use clarity::vm::types::PrincipalData;

use fake::Fake;
use fake::Faker;
use rand::rngs::OsRng;
use sbtc::deposits::CreateDepositRequest;
use sbtc::deposits::DepositInfo;
use sbtc::testing::regtest;
use sbtc::testing::regtest::Faucet;
use sbtc::testing::regtest::Recipient;
use signer::bitcoin::rpc::BitcoinCoreClient;
use signer::bitcoin::rpc::BitcoinTxInfo;
use signer::bitcoin::utxo;
use signer::bitcoin::utxo::SbtcRequests;
use signer::bitcoin::utxo::SignerBtcState;
use signer::bitcoin::utxo::SignerUtxo;
use signer::block_observer::Deposit;
use signer::config::Settings;
use signer::keys::PublicKey;
use signer::keys::SignerScriptPubKey;
use signer::message;
use signer::storage::model;
use signer::storage::model::BitcoinTxRef;
use signer::storage::model::EncryptedDkgShares;
use signer::storage::postgres::PgStore;
use signer::storage::DbWrite as _;

use crate::utxo_construction::generate_withdrawal;
use crate::utxo_construction::make_deposit_request;

/// A struct containing an actual deposit and a sweep transaction. The
/// sweep transaction was signed with the `signer` field's public key.
pub struct TestSweepSetup {
    /// The block hash of the bitcoin block that confirms the deposit
    /// transaction.
    pub deposit_block_hash: bitcoin::BlockHash,
    /// The full validated deposit info
    pub deposit_info: DepositInfo,
    /// Where the corresponding sBTC will be minted.
    pub deposit_recipient: PrincipalData,
    /// The deposit request, and a bitmap for how the signers voted on it.
    pub deposit_request: utxo::DepositRequest,
    /// The bitcoin transaction that the user made as a deposit for sBTC.
    pub deposit_tx_info: BitcoinTxInfo,
    /// The signer object. It's public key represents the group of signers'
    /// public keys, allowing us to abstract away the fact that there are
    /// many signers needed to sign a transaction.
    pub aggregated_signer: Recipient,
    /// The public keys of the signer set. It is effectively controlled by
    /// the above signer's private key.
    pub signer_keys: Vec<PublicKey>,
    /// The block hash of the bitcoin block that confirmed the sweep
    /// transaction.
    pub sweep_block_hash: bitcoin::BlockHash,
    /// The height of the bitcoin block that confirmed the sweep
    /// transaction.
    pub sweep_block_height: u64,
    /// The transaction that swept in the deposit transaction.
    pub sweep_tx_info: BitcoinTxInfo,
    /// The withdrawal request, and a bitmap for how the signers voted on
    /// it.
    pub withdrawal_request: utxo::WithdrawalRequest,
    /// The address that initiated with withdrawal request.
    pub withdrawal_sender: PrincipalData,
    /// This value affects whether a request is considered "accepted".
    /// During validation, a signer won't sign a transaction if it is not
    /// considered accepted but the collection of signers. Note that this
    /// threshold is the bitcoin signature threshold, which for v1 matches
    /// the signatures required on stacks.
    pub signatures_required: u16,
    /// Sweep transactions that have been broadcast but not yet stored.
    pub sweep_transactions: Vec<model::SweepTransaction>,
}

impl TestSweepSetup {
    /// Construct a new TestSweepSetup
    ///
    /// This is done as follows:
    /// 1. Generating a new "signer" and "depositor" objects that control
    ///    distinct private keys.
    /// 2. The depositor constructs and confirms a proper deposit
    ///    transaction, with a burn address on stacks as the recipient. The
    ///    max fee is the entire deposit.
    /// 3. Someone on the stacks network creates a withdrawal request to
    ///    sweep out funds.
    /// 4. The signer sweeps in the deposited funds and sweeps out the
    ///    withdrawal funds in a proper sweep transaction, that is also
    ///    confirmed on bitcoin.
    /// 5. Generate a set of "signer keys" that kinda represent the
    ///    signers. Transactions can be signed using only the private keys
    ///    of the "signer" from (1).
    pub fn new_setup<R>(rpc: &Client, faucet: &Faucet, amount: u64, rng: &mut R) -> Self
    where
        R: rand::Rng,
    {
        let signer = Recipient::new(AddressType::P2tr);
        let depositor = Recipient::new(AddressType::P2tr);
        let signers_public_key = signer.keypair.x_only_public_key().0;

        // Start off with some initial UTXOs to work with.
        faucet.send_to(100_000_000, &signer.address);
        faucet.send_to(50_000_000, &depositor.address);
        faucet.generate_blocks(1);

        // Now lets make a deposit transaction and submit it
        let depositor_utxo = depositor.get_utxos(rpc, None).pop().unwrap();

        more_asserts::assert_lt!(amount, 50_000_000);

        let (deposit_tx, deposit_request, deposit_info) =
            make_deposit_request(&depositor, amount, depositor_utxo, signers_public_key);
        rpc.send_raw_transaction(&deposit_tx).unwrap();
        let deposit_block_hash = faucet.generate_blocks(1).pop().unwrap();

        // This is randomly generated withdrawal request and the recipient
        // who can sign for the withdrawal UTXO.
        let (withdrawal_request, _withdrawal_recipient) = generate_withdrawal();
        // Okay now we try to peg-in the deposit by making a transaction.
        // Let's start by getting the signer's sole UTXO.
        let signer_utxo = signer.get_utxos(rpc, None).pop().unwrap();

        let mut requests = SbtcRequests {
            deposits: vec![deposit_request],
            withdrawals: vec![withdrawal_request],
            signer_state: SignerBtcState {
                utxo: SignerUtxo {
                    outpoint: OutPoint::new(signer_utxo.txid, signer_utxo.vout),
                    amount: signer_utxo.amount.to_sat(),
                    public_key: signers_public_key,
                },
                fee_rate: 10.0,
                public_key: signers_public_key,
                last_fees: None,
                magic_bytes: [b'T', b'3'],
            },
            accept_threshold: 4,
            num_signers: 7,
        };

        // There should only be one transaction here since there is only
        // one deposit request and no withdrawal requests.
        let (txid, sweep_transaction) = {
            let mut transactions = requests.construct_transactions().unwrap();
            assert_eq!(transactions.len(), 1);
            let mut unsigned = transactions.pop().unwrap();
            // Create the transaction package that we will store.
            let sweep_tx = message::SweepTransactionInfo::from_unsigned_at_block(
                // We expect the `sweep_block_hash` to be the block where the
                // sweep transaction was mined, so we use the deposit block hash
                // which is a previous block for the sweep broadcast.
                &deposit_block_hash,
                &unsigned,
            );

            // Add the signature and/or other required information to the
            // witness data.
            signer::testing::set_witness_data(&mut unsigned, signer.keypair);
            rpc.send_raw_transaction(&unsigned.tx).unwrap();
            // Return the txid and the sweep transaction.
            (unsigned.tx.compute_txid(), sweep_tx)
        };

        // Let's sweep in the transaction
        let sweep_block_hash = faucet.generate_blocks(1).pop().unwrap();
        let sweep_block_height =
            rpc.get_block_header_info(&sweep_block_hash).unwrap().height as u64;

        let settings = Settings::new_from_default_config().unwrap();
        let client = BitcoinCoreClient::try_from(&settings.bitcoin.rpc_endpoints[0]).unwrap();
        let sweep_tx_info = client
            .get_tx_info(&txid, &sweep_block_hash)
            .unwrap()
            .unwrap();

        let deposit_tx_info = client
            .get_tx_info(&deposit_tx.compute_txid(), &deposit_block_hash)
            .unwrap()
            .unwrap();

        TestSweepSetup {
            deposit_block_hash,
            deposit_info,
            deposit_recipient: PrincipalData::from(StacksAddress::burn_address(false)),
            deposit_request: requests.deposits.pop().unwrap(),
            deposit_tx_info,
            sweep_tx_info,
            sweep_block_height,
            sweep_block_hash,
            signer_keys: signer::testing::wallet::create_signers_keys(rng, &signer, 7),
            aggregated_signer: signer,
            withdrawal_request: requests.withdrawals.pop().unwrap(),
            withdrawal_sender: PrincipalData::from(StacksAddress::burn_address(false)),
            signatures_required: 2,
            sweep_transactions: vec![(&sweep_transaction).into()],
        }
    }

    /// Return the expected deposit request that our internal EmilyClient
    /// should return for the deposit here.
    pub fn emily_deposit_request(&self) -> CreateDepositRequest {
        CreateDepositRequest {
            outpoint: self.deposit_info.outpoint,
            reclaim_script: self.deposit_info.reclaim_script.clone(),
            deposit_script: self.deposit_info.deposit_script.clone(),
        }
    }

    /// Store all pending transaction packages to the database. Note that this
    /// is kept separate from [`Self::store_sweep_tx`] because this method
    /// represents the transaction package which was broadcast to the mempool
    /// (but not necessarily confirmed), while [`Self::store_sweep_tx`]
    /// represents the sweep transaction which has been observed on-chain.
    pub async fn store_sweep_transactions(&mut self, db: &PgStore) -> Vec<model::SweepTransaction> {
        let mut ret = vec![];
        for tx in self.sweep_transactions.drain(..) {
            db.write_sweep_transaction(&tx).await.unwrap();
            ret.push(tx.clone());
        }
        ret
    }

    /// Store the deposit transaction into the database
    pub async fn store_deposit_tx(&self, db: &PgStore) {
        let deposit_tx = model::Transaction {
            tx: bitcoin::consensus::serialize(&self.deposit_tx_info.tx),
            txid: self.deposit_tx_info.txid.to_byte_array(),
            tx_type: model::TransactionType::SbtcTransaction,
            block_hash: self.deposit_block_hash.to_byte_array(),
        };

        let bitcoin_tx_ref = BitcoinTxRef {
            txid: deposit_tx.txid.into(),
            block_hash: self.deposit_block_hash.into(),
        };

        db.write_transaction(&deposit_tx).await.unwrap();
        db.write_bitcoin_transaction(&bitcoin_tx_ref).await.unwrap();
    }
    /// Store the transaction that swept the deposit into the signers' UTXO
    /// into the database
    pub async fn store_sweep_tx(&self, db: &PgStore) {
        let sweep_tx = model::Transaction {
            tx: bitcoin::consensus::serialize(&self.sweep_tx_info.tx),
            txid: self.sweep_tx_info.txid.to_byte_array(),
            tx_type: model::TransactionType::SbtcTransaction,
            block_hash: self.sweep_block_hash.to_byte_array(),
        };

        let bitcoin_tx_ref = BitcoinTxRef {
            txid: sweep_tx.txid.into(),
            block_hash: sweep_tx.block_hash.into(),
        };

        db.write_transaction(&sweep_tx).await.unwrap();
        db.write_bitcoin_transaction(&bitcoin_tx_ref).await.unwrap();
    }

    /// Store the deposit request in the database.
    pub async fn store_deposit_request(&self, db: &PgStore) {
        let deposit = Deposit {
            tx_info: self.deposit_tx_info.clone(),
            info: self.deposit_info.clone(),
        };
        let deposit_request = model::DepositRequest::from(deposit);
        db.write_deposit_request(&deposit_request).await.unwrap();
    }

    /// Store how the signers voted on the deposit request.
    ///
    /// The deposit request must be stored in the database before this
    /// function is called.
    ///
    /// This function uses the `self.deposit_request.signer_bitmap` field
    /// to generate the corresponding deposit signer votes and then stores
    /// these decisions in the database.
    pub async fn store_deposit_decisions(&self, db: &PgStore) {
        let deposit_signers = self
            .signer_keys
            .iter()
            .copied()
            .zip(self.deposit_request.signer_bitmap)
            .map(|(signer_pub_key, is_rejected)| model::DepositSigner {
                txid: self.deposit_request.outpoint.txid.into(),
                output_index: self.deposit_request.outpoint.vout,
                signer_pub_key,
                is_accepted: !is_rejected,
                can_sign: true,
            });

        for decision in deposit_signers {
            db.write_deposit_signer_decision(&decision).await.unwrap();
        }
    }

    /// Use the bitmap in the `self.withdrawal_request.signer_bitmap` field to
    /// generate the corresponding deposit signer votes and store these
    /// decisions in the database.
    pub async fn store_withdrawal_decisions(&self, db: &PgStore) {
        let withdrawal_signers: Vec<model::WithdrawalSigner> = self
            .signer_keys
            .iter()
            .copied()
            .zip(self.deposit_request.signer_bitmap)
            .map(|(signer_pub_key, is_rejected)| model::WithdrawalSigner {
                request_id: self.withdrawal_request.request_id,
                block_hash: self.withdrawal_request.block_hash,
                txid: self.withdrawal_request.txid,
                signer_pub_key,
                is_accepted: !is_rejected,
            })
            .collect();

        for decision in withdrawal_signers {
            db.write_withdrawal_signer_decision(&decision)
                .await
                .unwrap();
        }
    }

    pub async fn store_withdrawal_request(&self, db: &PgStore) {
        let block = model::StacksBlock {
            block_hash: self.withdrawal_request.block_hash,
            block_height: self.sweep_block_height,
            parent_hash: Faker.fake_with_rng(&mut OsRng),
            bitcoin_anchor: self.sweep_block_hash.into(),
        };
        db.write_stacks_block(&block).await.unwrap();

        let withdrawal_request = model::WithdrawalRequest {
            request_id: self.withdrawal_request.request_id,
            txid: self.withdrawal_request.txid,
            block_hash: self.withdrawal_request.block_hash,
            recipient: self.withdrawal_request.clone().script_pubkey,
            amount: self.withdrawal_request.amount,
            max_fee: self.withdrawal_request.max_fee,
            sender_address: self.withdrawal_sender.clone().into(),
        };
        db.write_withdrawal_request(&withdrawal_request)
            .await
            .unwrap();
    }

    /// We need to have a row in the dkg_shares table for the scriptPubKey
    /// associated with the signers aggregate key.
    pub async fn store_dkg_shares(&self, db: &PgStore) {
        let aggregate_key: PublicKey = self.aggregated_signer.keypair.public_key().into();
        let shares = EncryptedDkgShares {
            script_pubkey: aggregate_key.signers_script_pubkey().into(),
            tweaked_aggregate_key: aggregate_key.signers_tweaked_pubkey().unwrap(),
            encrypted_private_shares: Vec::new(),
            public_shares: Vec::new(),
            aggregate_key,
            signer_set_public_keys: self.signer_keys.clone(),
            signature_share_threshold: self.signatures_required,
        };
        db.write_encrypted_dkg_shares(&shares).await.unwrap();
    }

    // This is all normal happy path things that need to happen in order to
    // pass validation of a stacks transaction.
    pub async fn store_happy_path_data(&mut self, db: &PgStore) {
        self.store_deposit_tx(&db).await;
        self.store_sweep_tx(&db).await;
        self.store_dkg_shares(&db).await;
        self.store_deposit_request(&db).await;
        self.store_deposit_decisions(&db).await;
        self.store_withdrawal_request(&db).await;
        self.store_sweep_transactions(&db).await;
    }
}

/// Fetch all block headers from bitcoin-core and store it in the database.
pub async fn backfill_bitcoin_blocks(db: &PgStore, rpc: &Client, chain_tip: &bitcoin::BlockHash) {
    let mut block_header = rpc.get_block_header_info(&chain_tip).unwrap();

    // There are no non-coinbase transactions below this height.
    while block_header.height as u64 >= regtest::MIN_BLOCKCHAIN_HEIGHT {
        let parent_header_hash = block_header.previous_block_hash.unwrap();
        let bitcoin_block = model::BitcoinBlock {
            block_hash: block_header.hash.into(),
            block_height: block_header.height as u64,
            parent_hash: parent_header_hash.into(),
        };

        db.write_bitcoin_block(&bitcoin_block).await.unwrap();
        block_header = rpc.get_block_header_info(&parent_header_hash).unwrap();
    }
}
