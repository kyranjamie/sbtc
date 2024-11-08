//! Database models for the signer.

use std::collections::BTreeSet;
use std::ops::Deref;

use bitcoin::hashes::Hash as _;
use bitvec::array::BitArray;
use blockstack_lib::chainstate::nakamoto::NakamotoBlock;
use clarity::vm::types::PrincipalData;
use serde::Deserialize;
use serde::Serialize;
use stacks_common::types::chainstate::{BurnchainHeaderHash, StacksBlockId};

use crate::block_observer::Deposit;
use crate::error::Error;
use crate::keys::PublicKey;
use crate::keys::PublicKeyXOnly;

/// Represents a single transaction which is part of a sweep transaction package
/// which has been broadcast to the Bitcoin network.
#[derive(Debug, Clone, PartialEq, PartialOrd, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct SweepTransaction {
    /// The Bitcoin transaction id.
    pub txid: BitcoinTxId,
    /// The transaction id of the signer UTXO consumed by this transaction.
    pub signer_prevout_txid: BitcoinTxId,
    /// The index of the signer UTXO consumed by this transaction.
    #[sqlx(try_from = "i32")]
    #[cfg_attr(feature = "testing", dummy(faker = "0..i32::MAX as u32"))]
    pub signer_prevout_output_index: u32,
    /// The amount of the signer UTXO consumed by this transaction.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "0..i32::MAX as u64"))]
    pub signer_prevout_amount: u64,
    /// The public key of the signer UTXO consumed by this transaction.
    pub signer_prevout_script_pubkey: ScriptPubKey,
    /// The total **output** amount of this transaction.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "0..i32::MAX as u64"))]
    pub amount: u64,
    /// The fee paid for this transaction.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "0..i32::MAX as u64"))]
    pub fee: u64,
    /// The Bitcoin block hash at which this transaction was created.
    pub created_at_block_hash: BitcoinBlockHash,
    /// The market fee rate at the time of this transaction.
    pub market_fee_rate: f64,
    /// The outputs created for the signers.
    #[sqlx(skip)]
    pub signer_outputs: Vec<SignerOutput>,
    /// List of deposits which were swept-in by this transaction.
    #[sqlx(skip)]
    pub swept_deposits: Vec<SweptDeposit>,
    /// List of withdrawals which were swept-out by this transaction.
    #[sqlx(skip)]
    pub swept_withdrawals: Vec<SweptWithdrawal>,
}

impl SweepTransaction {
    /// Return the outpoint of the signer's UTXO consumed by this transaction.
    pub fn signer_prevout_outpoint(&self) -> bitcoin::OutPoint {
        bitcoin::OutPoint {
            txid: self.signer_prevout_txid.into(),
            vout: self.signer_prevout_output_index,
        }
    }
}

impl From<&crate::message::SweepTransactionInfo> for SweepTransaction {
    fn from(info: &crate::message::SweepTransactionInfo) -> Self {
        Self {
            txid: info.txid.into(),
            signer_prevout_txid: info.signer_prevout_txid.into(),
            signer_prevout_output_index: info.signer_prevout_output_index,
            signer_prevout_amount: info.signer_prevout_amount,
            signer_prevout_script_pubkey: info.signer_prevout_script_pubkey.clone().into(),
            amount: info.amount,
            fee: info.fee,
            market_fee_rate: info.market_fee_rate,
            created_at_block_hash: info.created_at_block_hash.into(),
            signer_outputs: info.signer_outputs.iter().map(Into::into).collect(),
            swept_deposits: info.swept_deposits.iter().map(Into::into).collect(),
            swept_withdrawals: info.swept_withdrawals.iter().map(Into::into).collect(),
        }
    }
}

/// Represents a single deposit which has been swept-in by a sweep transaction.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct SweptDeposit {
    /// The index of the deposit input in the sBTC sweep transaction.
    #[sqlx(try_from = "i32")]
    #[cfg_attr(feature = "testing", dummy(faker = "0..i32::MAX as u32"))]
    pub input_index: u32,
    /// The Bitcoin txid of the deposit request UTXO being swept-in by this
    /// transaction.
    pub deposit_request_txid: BitcoinTxId,
    /// The Bitcoin output index of the deposit request UTXO being swept-in by
    /// this transaction.
    #[sqlx(try_from = "i32")]
    #[cfg_attr(feature = "testing", dummy(faker = "0..i32::MAX as u32"))]
    pub deposit_request_output_index: u32,
}

impl From<SweptDeposit> for bitcoin::OutPoint {
    fn from(deposit: SweptDeposit) -> Self {
        bitcoin::OutPoint {
            txid: deposit.deposit_request_txid.into(),
            vout: deposit.deposit_request_output_index,
        }
    }
}

impl From<&crate::message::SweptDeposit> for SweptDeposit {
    fn from(deposit: &crate::message::SweptDeposit) -> Self {
        Self {
            input_index: deposit.input_index,
            deposit_request_txid: deposit.deposit_request_txid.into(),
            deposit_request_output_index: deposit.deposit_request_output_index,
        }
    }
}

/// A transaction output controled by the signer.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct SignerOutput {
    /// The Bitcoin transaction id.
    pub txid: BitcoinTxId,
    /// The index of the output in the sBTC sweep transaction.
    #[sqlx(try_from = "i32")]
    #[cfg_attr(feature = "testing", dummy(faker = "0..i32::MAX as u32"))]
    pub output_index: u32,
    /// The scriptPubKey locking the output.
    pub script_pubkey: ScriptPubKey,
    /// The amount created in the output.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "1_000_000..1_000_000_000"))]
    pub amount: u64,
    /// The scriptPubKey locking the output.
    pub txo_type: TxoType,
}

impl From<&crate::message::SignerOutput> for SignerOutput {
    fn from(value: &crate::message::SignerOutput) -> Self {
        Self {
            txid: value.txid,
            output_index: value.output_index,
            script_pubkey: value.script_pubkey.clone(),
            amount: value.amount,
            txo_type: value.txo_type,
        }
    }
}

/// Represents a single withdrawal which has been swept-out by a sweep
/// transaction.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct SweptWithdrawal {
    /// The index of the withdrawal output in the sBTC sweep transaction.
    #[sqlx(try_from = "i32")]
    #[cfg_attr(feature = "testing", dummy(faker = "0..i32::MAX as u32"))]
    pub output_index: u32,
    /// The public request id of the withdrawal request serviced by this
    /// transaction.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "0..i64::MAX as u64"))]
    pub withdrawal_request_id: u64,
    /// The Stacks block hash of the Stacks block which included the withdrawal
    /// request transaction.
    pub withdrawal_request_block_hash: StacksBlockHash,
}

impl From<&crate::message::SweptWithdrawal> for SweptWithdrawal {
    fn from(withdrawal: &crate::message::SweptWithdrawal) -> Self {
        Self {
            output_index: withdrawal.output_index,
            withdrawal_request_id: withdrawal.withdrawal_request_id,
            withdrawal_request_block_hash: withdrawal.withdrawal_request_block_hash.into(),
        }
    }
}

/// Bitcoin block.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct BitcoinBlock {
    /// Block hash.
    pub block_hash: BitcoinBlockHash,
    /// Block height.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "0..i64::MAX as u64"))]
    pub block_height: u64,
    /// Hash of the parent block.
    pub parent_hash: BitcoinBlockHash,
}

impl From<&bitcoin::Block> for BitcoinBlock {
    fn from(block: &bitcoin::Block) -> Self {
        BitcoinBlock {
            block_hash: block.block_hash().into(),
            block_height: block
                .bip34_block_height()
                .expect("Failed to get block height"),
            parent_hash: block.header.prev_blockhash.into(),
        }
    }
}

impl From<bitcoin::Block> for BitcoinBlock {
    fn from(block: bitcoin::Block) -> Self {
        BitcoinBlock::from(&block)
    }
}

/// Stacks block.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct StacksBlock {
    /// Block hash.
    pub block_hash: StacksBlockHash,
    /// Block height.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "0..u32::MAX as u64"))]
    pub block_height: u64,
    /// Hash of the parent block.
    pub parent_hash: StacksBlockHash,
    /// The bitcoin block this stacks block is build upon (matching consensus hash)
    pub bitcoin_anchor: BitcoinBlockHash,
}

impl StacksBlock {
    /// Construct a StacksBlock from a NakamotoBlock and its bitcoin anchor
    pub fn from_nakamoto_block(block: &NakamotoBlock, bitcoin_anchor: &BitcoinBlockHash) -> Self {
        Self {
            block_hash: block.block_id().into(),
            block_height: block.header.chain_length,
            parent_hash: block.header.parent_block_id.into(),
            bitcoin_anchor: *bitcoin_anchor,
        }
    }
}

/// Deposit request.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct DepositRequest {
    /// Transaction ID of the deposit request transaction.
    pub txid: BitcoinTxId,
    /// Index of the deposit request UTXO.
    #[cfg_attr(feature = "testing", dummy(faker = "0..100"))]
    #[sqlx(try_from = "i32")]
    pub output_index: u32,
    /// Script spendable by the sBTC signers.
    pub spend_script: Bytes,
    /// Script spendable by the depositor.
    pub reclaim_script: Bytes,
    /// The address of which the sBTC should be minted,
    /// can be a smart contract address.
    pub recipient: StacksPrincipal,
    /// The amount in the deposit UTXO.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "1_000_000..1_000_000_000"))]
    pub amount: u64,
    /// The maximum portion of the deposited amount that may
    /// be used to pay for transaction fees.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "100..100_000"))]
    pub max_fee: u64,
    /// The relative lock time in the reclaim script.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "3..u16::MAX as u32"))]
    pub lock_time: u32,
    /// The public key used in the deposit script. The signers public key
    /// is for Schnorr signatures.
    pub signers_public_key: PublicKeyXOnly,
    /// The addresses of the input UTXOs funding the deposit request.
    #[cfg_attr(
        feature = "testing",
        dummy(faker = "crate::testing::dummy::BitcoinAddresses(1..5)")
    )]
    pub sender_script_pub_keys: Vec<ScriptPubKey>,
}

impl From<Deposit> for DepositRequest {
    fn from(deposit: Deposit) -> Self {
        let tx_input_iter = deposit.tx_info.vin.into_iter();
        // It's most likely the case that each of the inputs "came" from
        // the same Address, so we filter out duplicates.
        let sender_script_pub_keys: BTreeSet<ScriptPubKey> = tx_input_iter
            .map(|tx_in| ScriptPubKey::from_bytes(tx_in.prevout.script_pub_key.hex))
            .collect();

        Self {
            txid: deposit.info.outpoint.txid.into(),
            output_index: deposit.info.outpoint.vout,
            spend_script: deposit.info.deposit_script.to_bytes(),
            reclaim_script: deposit.info.reclaim_script.to_bytes(),
            recipient: deposit.info.recipient.into(),
            amount: deposit.info.amount,
            max_fee: deposit.info.max_fee,
            lock_time: deposit.info.lock_time.to_consensus_u32(),
            signers_public_key: deposit.info.signers_public_key.into(),
            sender_script_pub_keys: sender_script_pub_keys.into_iter().collect(),
        }
    }
}

impl DepositRequest {
    /// Return the outpoint associated with the deposit request.
    pub fn outpoint(&self) -> bitcoin::OutPoint {
        bitcoin::OutPoint {
            txid: self.txid.into(),
            vout: self.output_index,
        }
    }
}

/// A signer acknowledging a deposit request.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct DepositSigner {
    /// TxID of the deposit request.
    pub txid: BitcoinTxId,
    /// Output index of the deposit request.
    #[cfg_attr(feature = "testing", dummy(faker = "0..100"))]
    #[sqlx(try_from = "i32")]
    pub output_index: u32,
    /// Public key of the signer.
    pub signer_pub_key: PublicKey,
    /// Signals if the signer is prepared to sign for this request.
    pub is_accepted: bool,
    /// This specifies whether the indicated signer_pub_key can sign for
    /// the associated deposit request.
    pub can_sign: bool,
}

/// Withdraw request.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct WithdrawalRequest {
    /// Request ID of the withdrawal request. These are supposed to be
    /// unique, but there can be duplicates if there is a reorg that
    /// affects a transaction that calls the initiate-withdrawal-request
    /// public function.
    #[sqlx(try_from = "i64")]
    pub request_id: u64,
    /// The stacks transaction ID that lead to the creation of the
    /// withdrawal request.
    pub txid: StacksTxId,
    /// Stacks block ID of the block that includes the transaction
    /// associated with this withdrawal request.
    pub block_hash: StacksBlockHash,
    /// The address that should receive the BTC withdrawal.
    pub recipient: ScriptPubKey,
    /// The amount to withdraw.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "100..1_000_000_000"))]
    pub amount: u64,
    /// The maximum portion of the withdrawn amount that may
    /// be used to pay for transaction fees.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "100..10000"))]
    pub max_fee: u64,
    /// The address that initiated the request.
    pub sender_address: StacksPrincipal,
}

impl WithdrawalRequest {
    /// Return the identifier for the withdrawal request.
    pub fn qualified_id(&self) -> QualifiedRequestId {
        QualifiedRequestId {
            request_id: self.request_id,
            txid: self.txid,
            block_hash: self.block_hash,
        }
    }
}

/// A signer acknowledging a withdrawal request.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct WithdrawalSigner {
    /// Request ID of the withdrawal request.
    #[sqlx(try_from = "i64")]
    pub request_id: u64,
    /// The stacks transaction ID that lead to the creation of the
    /// withdrawal request.
    pub txid: StacksTxId,
    /// Stacks block hash of the withdrawal request.
    pub block_hash: StacksBlockHash,
    /// Public key of the signer.
    pub signer_pub_key: PublicKey,
    /// Signals if the signer is prepared to sign for this request.
    pub is_accepted: bool,
}

impl WithdrawalSigner {
    /// Return the identifier for the withdrawal request.
    pub fn qualified_id(&self) -> QualifiedRequestId {
        QualifiedRequestId {
            request_id: self.request_id,
            txid: self.txid,
            block_hash: self.block_hash,
        }
    }
}

/// A connection between a bitcoin block and a bitcoin transaction.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
pub struct BitcoinTxRef {
    /// Transaction ID.
    pub txid: BitcoinTxId,
    /// The block in which the transaction exists.
    pub block_hash: BitcoinBlockHash,
}

/// A connection between a bitcoin block and a bitcoin transaction.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct StacksTransaction {
    /// Transaction ID.
    pub txid: StacksTxId,
    /// The block in which the transaction exists.
    pub block_hash: StacksBlockHash,
}

/// For writing to the stacks_transactions or bitcoin_transactions table.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionIds {
    /// Transaction IDs.
    pub tx_ids: Vec<[u8; 32]>,
    /// The blocks in which the transactions exist.
    pub block_hashes: Vec<[u8; 32]>,
}

/// A raw transaction on either Bitcoin or Stacks.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct Transaction {
    /// Transaction ID.
    pub txid: [u8; 32],
    /// Encoded transaction.
    pub tx: Bytes,
    /// The type of the transaction.
    pub tx_type: TransactionType,
    /// The block id of the stacks block that includes this transaction
    pub block_hash: [u8; 32],
}

/// A deposit request with a response bitcoin transaction that has been
/// confirmed.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct SweptDepositRequest {
    /// The transaction ID of the bitcoin transaction that swept in the
    /// funds into the signers' UTXO.
    pub sweep_txid: BitcoinTxId,
    /// The block id of the bitcoin block that includes the sweep
    /// transaction.
    pub sweep_block_hash: BitcoinBlockHash,
    /// The block height of the block referenced by the `sweep_block_hash`.
    #[sqlx(try_from = "i64")]
    pub sweep_block_height: u64,
    /// Transaction ID of the deposit request transaction.
    pub txid: BitcoinTxId,
    /// Index of the deposit request UTXO.
    #[cfg_attr(feature = "testing", dummy(faker = "0..100"))]
    #[sqlx(try_from = "i32")]
    pub output_index: u32,
    /// The address of which the sBTC should be minted,
    /// can be a smart contract address.
    pub recipient: StacksPrincipal,
    /// The amount in the deposit UTXO.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "1_000_000..1_000_000_000"))]
    pub amount: u64,
    /// The maximum portion of the deposited amount that may
    /// be used to pay for transaction fees.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "100..100_000"))]
    pub max_fee: u64,
}

impl SweptDepositRequest {
    /// The OutPoint of the actual deposit
    pub fn deposit_outpoint(&self) -> bitcoin::OutPoint {
        bitcoin::OutPoint {
            txid: self.txid.into(),
            vout: self.output_index,
        }
    }
}

/// Withdraw request.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct SweptWithdrawalRequest {
    /// The transaction ID of the bitcoin transaction that swept out the
    /// funds to the intended recipient.
    pub sweep_txid: BitcoinTxId,
    /// The block id of the stacks block that includes this sweep
    /// transaction.
    pub sweep_block_hash: BitcoinBlockHash,
    /// Request ID of the withdrawal request. These are supposed to be
    /// unique, but there can be duplicates if there is a reorg that
    /// affects a transaction that calls the `initiate-withdrawal-request`
    /// public function.
    #[sqlx(try_from = "i64")]
    pub request_id: u64,
    /// The stacks transaction ID that lead to the creation of the
    /// withdrawal request.
    pub txid: StacksTxId,
    /// Stacks block ID of the block that includes the transaction
    /// associated with this withdrawal request.
    pub block_hash: StacksBlockHash,
    /// The ScriptPubKey that should receive the BTC withdrawal.
    pub recipient: ScriptPubKey,
    /// The amount of satoshis to withdraw.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "100..1_000_000_000"))]
    pub amount: u64,
    /// The maximum amount that may be spent as for the bitcoin miner
    /// transaction fee.
    #[sqlx(try_from = "i64")]
    #[cfg_attr(feature = "testing", dummy(faker = "100..10000"))]
    pub max_fee: u64,
    /// The stacks address that initiated the request. This is populated
    /// using `tx-sender`.
    pub sender_address: StacksPrincipal,
}

/// Persisted DKG shares
///
/// This struct represents the output of a successful run of distributed
/// key generation (DKG) that was run by a set of signers.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct EncryptedDkgShares {
    /// The aggregate key for these shares
    pub aggregate_key: PublicKey,
    /// The tweaked aggregate key for these shares
    pub tweaked_aggregate_key: PublicKey,
    /// The `scriptPubKey` for the aggregate public key.
    pub script_pubkey: ScriptPubKey,
    /// The encrypted DKG shares
    pub encrypted_private_shares: Bytes,
    /// The public DKG shares
    pub public_shares: Bytes,
    /// The set of public keys that were a party to the DKG.
    pub signer_set_public_keys: Vec<PublicKey>,
    /// The threshold number of signature shares required to generate a
    /// Schnorr signature.
    ///
    /// In WSTS each signer may contribute a fixed portion of a single
    /// signature. This value specifies the total number of portions
    /// (shares) that are needed in order to construct a signature.
    #[sqlx(try_from = "i32")]
    pub signature_share_threshold: u16,
}

/// Persisted public DKG shares from other signers
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct RotateKeysTransaction {
    /// Transaction ID.
    pub txid: StacksTxId,
    /// The aggregate key for these shares.
    ///
    /// TODO(511): maybe make the aggregate key private. Set it using the
    /// `signer_set`, ensuring that it cannot drift from the given keys.
    pub aggregate_key: PublicKey,
    /// The public keys of the signers.
    pub signer_set: Vec<PublicKey>,
    /// The number of signatures required for the multi-sig wallet.
    #[sqlx(try_from = "i32")]
    pub signatures_required: u16,
}

/// A struct containing how a signer voted for a deposit or withdrawal
/// request.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub struct SignerVote {
    /// The public key of the signer that cast the vote.
    pub signer_public_key: PublicKey,
    /// How the signer voted for a transaction. None is returned if we do
    /// not have a record of how the signer voted
    pub is_accepted: Option<bool>,
}

/// How the signers voted on a thing.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SignerVotes(Vec<SignerVote>);

impl Deref for SignerVotes {
    type Target = [SignerVote];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<SignerVote>> for SignerVotes {
    fn from(mut votes: Vec<SignerVote>) -> Self {
        votes.sort_by_key(|vote| vote.signer_public_key);
        SignerVotes(votes)
    }
}

impl From<SignerVotes> for BitArray<[u8; 16]> {
    fn from(votes: SignerVotes) -> BitArray<[u8; 16]> {
        let mut signer_bitmap = BitArray::ZERO;
        votes
            .iter()
            .enumerate()
            .take(signer_bitmap.len().min(crate::MAX_KEYS as usize))
            .for_each(|(index, vote)| {
                // The BitArray::<[u8; 16]>::set function panics if the
                // index is out of bounds but that cannot be the case here
                // because we only take 128 values.
                //
                // Note that the signer bitmap here is true for votes
                // *against*, and a missing vote is an implicit vote
                // against.
                signer_bitmap.set(index, !vote.is_accepted.unwrap_or(false));
            });

        signer_bitmap
    }
}

/// The types of transactions the signer is interested in.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::Type, strum::Display)]
#[sqlx(type_name = "transaction_type", rename_all = "snake_case")]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
#[strum(serialize_all = "snake_case")]
pub enum TransactionType {
    /// An sBTC transaction on Bitcoin.
    SbtcTransaction,
    /// A deposit request transaction on Bitcoin.
    DepositRequest,
    /// A withdrawal request transaction on Stacks.
    WithdrawRequest,
    /// A deposit accept transaction on Stacks.
    DepositAccept,
    /// A withdrawal accept transaction on Stacks.
    WithdrawAccept,
    /// A withdraw reject transaction on Stacks.
    WithdrawReject,
    /// A rotate keys call on Stacks.
    RotateKeys,
    /// A donation to signers aggregated key on Bitcoin.
    Donation,
}

/// The types of transactions the signer is interested in.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, sqlx::Type, strum::Display)]
#[sqlx(type_name = "txo_type", rename_all = "snake_case")]
#[derive(serde::Serialize, serde::Deserialize)]
#[strum(serialize_all = "snake_case")]
#[cfg_attr(feature = "testing", derive(fake::Dummy))]
pub enum TxoType {
    /// An sBTC transaction on Bitcoin.
    Signers,
    /// A donation to signers aggregated key on Bitcoin.
    Donation,
}

/// An identifier for a withdrawal request, comprised of the Stacks
/// transaction ID, the Stacks block ID that included the transaction, and
/// the request-id generated by the clarity contract for the withdrawal
/// request.
///
/// We need all three IDs because a transaction can be included in more
/// than one stacks block (because of reorgs), and a transaction can
/// generate more than one withdrawal request, so we need the request-id.
///
/// A request-id and a Stacks Block ID is enough to uniquely identify the
/// request, but we add in the transaction ID for completeness.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QualifiedRequestId {
    /// The Stacks block ID that includes the transaction that generated
    /// the request.
    pub block_hash: StacksBlockHash,
    /// The txid that generated the request.
    pub txid: StacksTxId,
    /// The ID that was generated in the clarity contract call for the
    /// withdrawal request.
    pub request_id: u64,
}

/// A bitcoin transaction
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BitcoinTx(bitcoin::Transaction);

impl Deref for BitcoinTx {
    type Target = bitcoin::Transaction;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<bitcoin::Transaction> for BitcoinTx {
    fn from(value: bitcoin::Transaction) -> Self {
        Self(value)
    }
}

impl From<BitcoinTx> for bitcoin::Transaction {
    fn from(value: BitcoinTx) -> Self {
        value.0
    }
}

/// The bitcoin transaction ID
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BitcoinTxId(bitcoin::Txid);

impl Deref for BitcoinTxId {
    type Target = bitcoin::Txid;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl BitcoinTxId {
    /// Return the inner bytes for the block hash
    pub fn into_bytes(&self) -> [u8; 32] {
        self.0.to_byte_array()
    }
}

impl From<bitcoin::Txid> for BitcoinTxId {
    fn from(value: bitcoin::Txid) -> Self {
        Self(value)
    }
}

impl From<BitcoinTxId> for bitcoin::Txid {
    fn from(value: BitcoinTxId) -> Self {
        value.0
    }
}

impl From<[u8; 32]> for BitcoinTxId {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bitcoin::Txid::from_byte_array(bytes))
    }
}

impl std::fmt::Display for BitcoinTxId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Bitcoin block hash
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BitcoinBlockHash(bitcoin::BlockHash);

impl BitcoinBlockHash {
    /// Return the inner bytes for the block hash
    pub fn into_bytes(&self) -> [u8; 32] {
        self.0.to_byte_array()
    }
}

impl AsRef<[u8; 32]> for BitcoinBlockHash {
    fn as_ref(&self) -> &[u8; 32] {
        self.0.as_ref()
    }
}

impl Deref for BitcoinBlockHash {
    type Target = bitcoin::BlockHash;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<bitcoin::BlockHash> for BitcoinBlockHash {
    fn from(value: bitcoin::BlockHash) -> Self {
        Self(value)
    }
}

impl From<&BitcoinBlockHash> for bitcoin::BlockHash {
    fn from(value: &BitcoinBlockHash) -> Self {
        value.0
    }
}

impl From<BitcoinBlockHash> for bitcoin::BlockHash {
    fn from(value: BitcoinBlockHash) -> Self {
        value.0
    }
}

impl From<[u8; 32]> for BitcoinBlockHash {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bitcoin::BlockHash::from_byte_array(bytes))
    }
}

impl From<BurnchainHeaderHash> for BitcoinBlockHash {
    fn from(value: BurnchainHeaderHash) -> Self {
        let mut bytes = value.into_bytes();
        bytes.reverse();
        bytes.into()
    }
}

impl From<BitcoinBlockHash> for BurnchainHeaderHash {
    fn from(value: BitcoinBlockHash) -> Self {
        let mut bytes = value.to_byte_array();
        bytes.reverse();
        BurnchainHeaderHash(bytes)
    }
}

impl std::fmt::Display for BitcoinBlockHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A struct that references a specific bitcoin block is identifier and its
/// position in the blockchain.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BitcoinBlockRef {
    /// The height of the block in the bitcoin blockchain.
    pub block_height: u64,
    /// Bitcoin block hash. It uniquely identifies the bitcoin block.
    pub block_hash: BitcoinBlockHash,
}

impl From<BitcoinBlock> for BitcoinBlockRef {
    fn from(value: BitcoinBlock) -> Self {
        Self::from(&value)
    }
}

impl From<&BitcoinBlock> for BitcoinBlockRef {
    fn from(value: &BitcoinBlock) -> Self {
        Self {
            block_hash: value.block_hash,
            block_height: value.block_height,
        }
    }
}

/// The Stacks block ID. This is different from the block header hash.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StacksBlockHash(StacksBlockId);

impl Deref for StacksBlockHash {
    type Target = StacksBlockId;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<StacksBlockId> for StacksBlockHash {
    fn from(value: StacksBlockId) -> Self {
        Self(value)
    }
}

impl From<StacksBlockHash> for StacksBlockId {
    fn from(value: StacksBlockHash) -> Self {
        value.0
    }
}

impl From<[u8; 32]> for StacksBlockHash {
    fn from(bytes: [u8; 32]) -> Self {
        Self(StacksBlockId(bytes))
    }
}

/// Stacks transaction ID
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StacksTxId(blockstack_lib::burnchains::Txid);

impl std::fmt::Display for StacksTxId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for StacksTxId {
    type Target = blockstack_lib::burnchains::Txid;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<blockstack_lib::burnchains::Txid> for StacksTxId {
    fn from(value: blockstack_lib::burnchains::Txid) -> Self {
        Self(value)
    }
}

impl From<StacksTxId> for blockstack_lib::burnchains::Txid {
    fn from(value: StacksTxId) -> Self {
        value.0
    }
}

impl From<[u8; 32]> for StacksTxId {
    fn from(bytes: [u8; 32]) -> Self {
        Self(blockstack_lib::burnchains::Txid(bytes))
    }
}

/// A stacks address. It can be either a smart contract address or a
/// standard address.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StacksPrincipal(PrincipalData);

impl Deref for StacksPrincipal {
    type Target = PrincipalData;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::str::FromStr for StacksPrincipal {
    type Err = Error;
    fn from_str(literal: &str) -> Result<Self, Self::Err> {
        let principal = PrincipalData::parse(literal).map_err(Error::ParsePrincipalData)?;
        Ok(Self(principal))
    }
}

impl From<PrincipalData> for StacksPrincipal {
    fn from(value: PrincipalData) -> Self {
        Self(value)
    }
}

impl From<StacksPrincipal> for PrincipalData {
    fn from(value: StacksPrincipal) -> Self {
        value.0
    }
}

impl Ord for StacksPrincipal {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (&self.0, &other.0) {
            (PrincipalData::Contract(x), PrincipalData::Contract(y)) => x.cmp(y),
            (PrincipalData::Standard(x), PrincipalData::Standard(y)) => x.cmp(y),
            (PrincipalData::Standard(x), PrincipalData::Contract(y)) => {
                x.cmp(&y.issuer).then(std::cmp::Ordering::Less)
            }
            (PrincipalData::Contract(x), PrincipalData::Standard(y)) => {
                x.issuer.cmp(y).then(std::cmp::Ordering::Greater)
            }
        }
    }
}

impl PartialOrd for StacksPrincipal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A ScriptPubkey of a UTXO.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScriptPubKey(bitcoin::ScriptBuf);

impl Deref for ScriptPubKey {
    type Target = bitcoin::ScriptBuf;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<bitcoin::ScriptBuf> for ScriptPubKey {
    fn from(value: bitcoin::ScriptBuf) -> Self {
        Self(value)
    }
}

impl From<ScriptPubKey> for bitcoin::ScriptBuf {
    fn from(value: ScriptPubKey) -> Self {
        value.0
    }
}

impl ScriptPubKey {
    /// Converts byte vector into script.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        bitcoin::ScriptBuf::from_bytes(bytes).into()
    }
}

/// Arbitrary bytes
pub type Bytes = Vec<u8>;

#[cfg(test)]
mod tests {
    use fake::Fake;
    use rand::SeedableRng;

    use super::*;

    #[test]
    fn conversion_bitcoin_header_hashes() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);

        let block_hash: BitcoinBlockHash = fake::Faker.fake_with_rng(&mut rng);
        let stacks_hash = BurnchainHeaderHash::from(block_hash);
        let round_trip = BitcoinBlockHash::from(stacks_hash);
        assert_eq!(block_hash, round_trip);

        let stacks_hash = BurnchainHeaderHash(fake::Faker.fake_with_rng(&mut rng));
        let block_hash = BitcoinBlockHash::from(stacks_hash);
        let round_trip = BurnchainHeaderHash::from(block_hash);
        assert_eq!(stacks_hash, round_trip);
    }
}
