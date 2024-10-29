//! validation of bitcoin transactions.

// use std::ops::Deref;

use bitcoin::Amount;
use bitcoin::OutPoint;
use bitcoin::TxOut;

use crate::context::Context;
use crate::error::Error;
use crate::keys::PublicKey;
use crate::storage::model::BitcoinBlockHash;
use crate::storage::model::BitcoinTx;
use crate::storage::model::QualifiedRequestId;
use crate::storage::model::ScriptPubKey;
use crate::storage::DbRead;

/// The necessary information for validating a bitcoin transaction.
#[derive(Debug, Clone)]
pub struct BitcoinTxContext {
    /// This signer's current view of the chain tip of the canonical
    /// bitcoin blockchain. It is the block hash and height of the block on
    /// the bitcoin blockchain with the greatest height. On ties, we sort
    /// by the block hash descending and take the first one.
    pub chain_tip: BitcoinBlockHash,
    /// How many bitcoin blocks back from the chain tip the signer will
    /// look for requests.
    pub tx: BitcoinTx,
    /// The requests
    pub request_ids: Vec<QualifiedRequestId>,
    /// The public key of the signer that created the deposit request
    /// transaction. This is very unlikely to ever be used in the
    /// [`BitcoinTx::validate`] function, but is here for logging and
    /// tracking purposes.
    pub origin: PublicKey,
}

impl BitcoinTxContext {
    /// 1. All deposit requests consumed by the bitcoin transaction are
    ///    accepted by the signer.
    /// 2. All withdraw requests fulfilled by the bitcoin transaction are
    ///    accepted by the signer.
    /// 3. The apportioned transaction fee for each request does not exceed
    ///    any max_fee.
    /// 4. All transaction inputs are spendable by the signers.
    /// 5. Any transaction outputs that aren't fulfilling withdraw requests
    ///    are spendable by the signers or unspendable.
    /// 6. Each deposit request input has an associated amount that is
    ///    greater than their assessed fee.
    /// 7. There is at least 2 blocks and 2 hours of lock-time left before
    ///    the depositor can reclaim their funds.
    /// 8. Each deposit is on the canonical bitcoin blockchain.
    pub async fn validate<C>(&self, ctx: &C) -> Result<(), Error>
    where
        C: Context + Send + Sync,
    {
        let signer_amount = self.validate_signer_input(ctx).await?;
        let deposit_amounts = self.validate_deposits(ctx).await?;

        self.validate_signer_outputs(ctx).await?;
        self.validate_withdrawals(ctx).await?;

        let input_amounts = signer_amount + deposit_amounts;

        self.validate_fees(input_amounts)?;
        Ok(())
    }

    fn validate_fees(&self, _input_amounts: Amount) -> Result<(), Error> {
        let _output_amounts = self.tx.output.iter().map(|tx_out| tx_out.value).sum::<Amount>();

        Ok(())
    }

    ///
    async fn validate_signer_input<C>(&self, ctx: &C) -> Result<Amount, Error>
    where
        C: Context + Send + Sync,
    {
        let db = ctx.get_storage();
        let Some(signer_txo_input) = self.tx.input.first() else {
            return Err(BitcoinInputError::InvalidSignerUtxo.into_error(self));
        };
        let signer_txo_txid = signer_txo_input.previous_output.txid.into();

        let Some(signer_tx) = db.get_bitcoin_tx(&signer_txo_txid).await? else {
            return Err(BitcoinInputError::InvalidSignerUtxo.into_error(self));
        };

        let output_index = signer_txo_input
            .previous_output
            .vout
            .try_into()
            .map_err(|_| Error::TypeConversion)?;
        let signer_prevout_utxo = signer_tx
            .tx_out(output_index)
            .map_err(Error::BitcoinOutputIndex)?;
        let script = signer_prevout_utxo.script_pubkey.clone().into();

        if !db.is_signer_script_pub_key(&script).await? {
            return Err(BitcoinInputError::InvalidSignerUtxo.into_error(self));
        }

        Ok(signer_prevout_utxo.value)
    }

    ///
    async fn validate_signer_outputs<C>(&self, ctx: &C) -> Result<(), Error>
    where
        C: Context + Send + Sync,
    {
        let db = ctx.get_storage();
        let Some(signer_txo_output) = self.tx.output.first() else {
            return Err(BitcoinOutputError::InvalidSignerUtxo.into_error(self));
        };

        let script = signer_txo_output.script_pubkey.clone().into();

        if !db.is_signer_script_pub_key(&script).await? {
            return Err(BitcoinOutputError::InvalidSignerUtxo.into_error(self));
        }

        Ok(())
    }

    ///
    async fn validate_deposits<C>(&self, ctx: &C) -> Result<Amount, Error>
    where
        C: Context + Send + Sync,
    {
        let db = ctx.get_storage();
        let signer_public_key = PublicKey::from_private_key(&ctx.config().signer.private_key);
        // 1. All deposit requests consumed by the bitcoin transaction are
        //    accepted by the signer.

        let mut deposit_amount = 0;

        for tx_in in self.tx.input.iter().skip(1) {
            let outpoint = tx_in.previous_output;
            let report = db
                .get_deposit_request_report(
                    &self.chain_tip,
                    &outpoint.txid.into(),
                    outpoint.vout,
                    &signer_public_key,
                )
                .await?;

            deposit_amount += report.amount.unwrap_or(0);

            report
                .validate(outpoint)
                .map_err(|err| err.into_error(self))?;
        }

        Ok(Amount::from_sat(deposit_amount))
    }

    ///
    async fn validate_withdrawals<C>(&self, ctx: &C) -> Result<(), Error>
    where
        C: Context + Send + Sync,
    {
        let db = ctx.get_storage();

        if self.tx.output.len() != self.request_ids.len() + 2 {
            return Err(BitcoinOutputError::UnexpectedOutput.into_error(self));
        }

        let withdrawal_iter = self.tx.output.iter().skip(2).zip(self.request_ids.iter());
        for (utxo, req_id) in withdrawal_iter {
            let report = db.get_withdrawal_request(&req_id).await?;

            report
                .validate(utxo, req_id)
                .map_err(|err| err.into_error(self))?;
        }
        Ok(())
    }
}

/// The responses for validation of a sweep transaction on bitcoin.
#[derive(Debug, thiserror::Error, PartialEq, Eq, Copy, Clone)]
pub enum BitcoinInputError {
    ///
    #[error("the assessed fee for a deposit would exceed their max-fee")]
    AssessedDepositFeeTooHigh,
    /// The signer is not part of the signer set that generated the
    /// aggregate public key used to lock the deposit funds.
    #[error("the signer is not part of the signing set for the aggregate public key")]
    CannotSignDepositRequest,
    /// The deposit transaction has been been confirmed on a bitcoin block
    /// that is not part of the canonical bitcoin blockchain.
    #[error("deposit transaction not on canonical bitcoin blockchain")]
    DepositTxReorged,
    ///
    #[error("deposit transaction not on canonical bitcoin blockchain")]
    DepositUtxoSpent,
    /// The signers' UTXO is not locked with the latest aggregate public
    /// key.
    #[error("signers' UTXO locked with incorrect scriptPubKey")]
    InvalidSignerUtxo,
    /// Given the current time and block height, it would be imprudent to
    /// attempt to sweep in a deposit request with the given lock-time.
    #[error("lock-time expiration is too soon")]
    LockTimeExpirationTooSoon,
    /// The signer does not have a record of the deposit request in our
    /// database.
    #[error("the signer does not have a record of the deposit request")]
    NoDepositRequestVote,
    /// The signer has rejected the deposit request.
    #[error("the signer has not accepted the deposit request")]
    RejectedDepositRequest,
    /// The signer does not have a record of the deposit request in our
    /// database.
    #[error("the signer does not have a record of the deposit request")]
    UnknownDepositRequest,
}

/// The responses for validation of a sweep transaction on bitcoin.
#[derive(Debug, thiserror::Error, PartialEq, Eq, Copy, Clone)]
pub enum BitcoinOutputError {
    ///
    #[error("the assessed fee for a withdrawal would exceed their max-fee")]
    AssessedWithdrawalFeeTooHigh,
    /// One of the output amounts does not match the amount in the withdrawal request.
    #[error("the amount in the withdrawal request does not match the amount in our records")]
    IncorrectWithdrawalAmount,
    /// One of the output amounts does not match the amount in the withdrawal request.
    #[error("the scriptPubKey for a withdrawal UTXO does not match the one in our records")]
    IncorrectWithdrawalRecipient,
    /// The signers' UTXO is not locked with the latest aggregate public
    /// key.
    #[error("signers' UTXO locked with incorrect scriptPubKey")]
    InvalidSignerUtxo,
    /// All UTXOs must be either the signers, an OP_RETURN UTXO with zero
    /// amount, or a UTXO servicing a withdrawal request.
    #[error("one of the UTXOs is unexpected")]
    InvalidUtxo,
    /// The OP_RETURN UTXO must have an amount of zero and include the
    /// expected signer bitmap, and merkle tree.
    #[error("signers' OP_RETRUN output does not match what is expected")]
    InvalidOpReturnOutput,
    /// The signer does not have a record of the deposit request in our
    /// database.
    #[error("the signer does not have a record of the deposit request")]
    NoWithdrawalRequestVote,
    /// The signer has rejected the deposit request.
    #[error("the signer has not accepted the deposit request")]
    RejectedWithdrawalRequest,
    /// One of the output amounts does not match the amount in the withdrawal request.
    #[error("the signer does not have a recored of the withdrawal request")]
    UnknownWithdrawalRequest,
    /// One of the output amounts does not match the amount in the withdrawal request.
    #[error("the signer does not have a recored of the withdrawal request")]
    UnexpectedOutput,
}

/// The responses for validation of a sweep transaction on bitcoin.
#[derive(Debug, thiserror::Error, PartialEq, Eq, Copy, Clone)]
pub enum BitcoinSweepErrorMsg {
    ///
    #[error("the assessed fee for a deposit would exceed their max-fee")]
    Input(BitcoinInputError),
    ///
    #[error("the assessed fee for a withdrawal would exceed their max-fee")]
    Output(BitcoinOutputError),
}

/// A struct for a bitcoin validation error containing all the necessary
/// context.
#[derive(Debug)]
pub struct BitcoinValidationError {
    /// The specific error that happened during validation.
    pub error: BitcoinSweepErrorMsg,
    /// The additional information that was used when trying to
    /// validate the complete-deposit contract call. This includes the
    /// public key of the signer that was attempting to generate the
    /// `complete-deposit` transaction.
    pub context: BitcoinTxContext,
}

impl std::fmt::Display for BitcoinValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO(191): Add the other variables to the error message.
        self.error.fmt(f)
    }
}

impl std::error::Error for BitcoinValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl BitcoinInputError {
    fn into_error(self, ctx: &BitcoinTxContext) -> Error {
        Error::BitcoinValidation(Box::new(BitcoinValidationError {
            error: BitcoinSweepErrorMsg::Input(self),
            context: ctx.clone(),
        }))
    }
}

impl BitcoinOutputError {
    fn into_error(self, ctx: &BitcoinTxContext) -> Error {
        Error::BitcoinValidation(Box::new(BitcoinValidationError {
            error: BitcoinSweepErrorMsg::Output(self),
            context: ctx.clone(),
        }))
    }
}

/// An enum for the confirmation status of a deposit request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepositRequestConfirmationStatus {
    /// We have no record of the deposit request.
    NoRecord,
    /// We have a record of the deposit request transaction, and it has been
    /// confirmed on the canonical bitcoin blockchain. It remains unspent.
    Confirmed,
    /// We have a record of the deposit request being included as an input
    /// in another bitcoin transaction that has been confirmed on the
    /// canonical bitcoin blockchain.
    Spent,
    /// We have a record of the deposit request transaction, and it has not
    /// been confirmed on the canonical bitcoin blockchain.
    ///
    /// Under normal operation, we will almost certainly have a record of a
    /// deposit request, and we require that the deposit transaction be
    /// confirmed before we write it to our database. But the deposit
    /// transaction can be affected by a bitcoin reorg, where it is no
    /// longer confirmed on the canonical bitcoin blockchain. If this
    /// happens when we query for the status then it will come back as
    /// unconfirmed.
    Unconfirmed,
}

/// A struct for the a status report summary of a deposit request for use
/// in validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepositRequestReport {
    /// The confirmation status of the deposit request transaction.
    pub status: DepositRequestConfirmationStatus,
    /// Whether this signer was part of the signing set associated with the
    /// deposited funds. If the signer is not part of the signing set, then
    /// we do not do a check of whether we will accept it otherwise.
    ///
    /// This should only be None if we do not have a record of the deposit
    /// request.
    pub can_sign: Option<bool>,
    /// Whether this signer accepted the deposit request or not. This
    /// should only be None if we do not have a record of the deposit
    /// request or if we cannot sign for the deposited funds.
    pub is_accepted: Option<bool>,
    /// The deposit amount
    pub amount: Option<u64>,
}

impl DepositRequestReport {
    fn validate(self, _: OutPoint) -> Result<(), BitcoinInputError> {
        match self.status {
            DepositRequestConfirmationStatus::NoRecord => {
                return Err(BitcoinInputError::UnknownDepositRequest);
            }
            DepositRequestConfirmationStatus::Unconfirmed => {
                return Err(BitcoinInputError::DepositTxReorged);
            }
            DepositRequestConfirmationStatus::Spent => {
                return Err(BitcoinInputError::DepositUtxoSpent);
            }
            DepositRequestConfirmationStatus::Confirmed => (),
        };

        if self.can_sign.is_none() {
            return Err(BitcoinInputError::NoDepositRequestVote);
        }

        if self.can_sign != Some(true) {
            return Err(BitcoinInputError::CannotSignDepositRequest);
        }

        if self.is_accepted != Some(true) {
            return Err(BitcoinInputError::RejectedDepositRequest);
        }

        Ok(())
    }
}

/// An enum for the confirmation status of a deposit request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WithdrawalRequestConfirmationStatus {
    /// We have no record of the deposit request.
    NoRecord,
    /// We have a record of the withdrawal request event, and it has been
    /// confirmed on the canonical stacks blockchain. It remains
    /// unfulfilled.
    Confirmed,
    /// We have a record of the withdrawal request event, and it has been
    /// confirmed on the canonical stacks blockchain. It has been
    /// fulfilled.
    Fulfilled,
}

/// A struct for the a status report summary of a deposit request for use
/// in validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawalRequestReport {
    /// The confirmation status of the deposit request transaction.
    pub status: WithdrawalRequestConfirmationStatus,
    /// Whether this signer was part of the signing set associated with the
    /// deposited funds. If the signer is not part of the signing set, then
    /// we do not do a check of whether we will accept it otherwise.
    ///
    /// This should only be None if we do not have a record of the deposit
    /// request.
    pub amount: Option<u64>,
    /// Whether this signer accepted the deposit request or not. This
    /// should only be None if we do not have a record of the deposit
    /// request or if we cannot sign for the deposited funds.
    pub recipient: Option<ScriptPubKey>,
    /// request or if we cannot sign for the deposited funds.
    pub max_fee: Option<u64>,
}

impl WithdrawalRequestReport {
    fn validate(self, utxo: &TxOut, _: &QualifiedRequestId) -> Result<(), BitcoinOutputError> {
        match self.status {
            WithdrawalRequestConfirmationStatus::NoRecord => {
                return Err(BitcoinOutputError::UnknownWithdrawalRequest);
            }
            WithdrawalRequestConfirmationStatus::Fulfilled => {
                return Err(BitcoinOutputError::UnknownWithdrawalRequest);
            }
            WithdrawalRequestConfirmationStatus::Confirmed => (),
        };

        if self.amount != Some(utxo.value.to_sat()) {
            return Err(BitcoinOutputError::IncorrectWithdrawalAmount);
        }

        if self.recipient.as_deref() != Some(&utxo.script_pubkey) {
            return Err(BitcoinOutputError::IncorrectWithdrawalRecipient);
        }

        Ok(())
    }
}
