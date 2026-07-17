//! Transaction assembler — bridges `opticrum_calculator` Instruction builders
//! with the server's `ChainProvider`, `Signer`, and `RpcClient`.
//!
//! This is the production transaction pipeline for seller-side operations.
//! Buyer-side operations (create/cancel order) are handled by the frontend
//! application directly.
//!
//! ## Architecture
//!
//! ```text
//! API handler
//!   → service function (e.g. match_order)
//!     → TransactionAssembler::match_order()
//!       → opticrum_calculator::match_order()  // builds Instruction<T>
//!       → TransactionCalculator::new_skeleton() // builds TransactionSkeleton
//!       → balance_and_sign()                   // adds secp256k1 signatures
//!       → TransactionCalculator::apply_skeleton()
//!       → send_registered_and_wait()           // sends, registers, waits
//!       → return tx_hash
//! ```
//!
//! Confirm-timeout is sourced from cinnabar skeleton's send_and_wait implementation
//! with the send/confirm split, so we can notify the frontend with the hash as soon
//! as the transaction has been broadcasted.

use ckb_cinnabar_calculator::{
    address::Address,
    instruction::{predefined::balance_and_sign, DefaultInstruction, TransactionCalculator},
    operation::basic::AddSecp256k1SighashCellDep,
    re_exports::ckb_jsonrpc_types::Status,
    re_exports::ckb_types::H256,
    rpc::{RpcClient, RPC},
    skeleton::TransactionSkeleton,
};
use opticrum_calculator::{
    calculator as opticrum_calc,
    types::{MatchArgs, MatchInfo, OrderInfo},
};
use secp256k1::SecretKey;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::error::AppError;
use crate::services::pending_txs::PendingTxRegistry;

/// Maximum time to wait for a transaction to be committed.
/// Prevents the server from hanging indefinitely if the CKB node
/// never mines the transaction.
const CONFIRM_TIMEOUT_SECS: u64 = 300;

/// High-level transaction assembler that produces real on-chain transactions.
///
/// Created once at startup with a concrete `RpcClient` (the generic `T: RPC`
/// bound required by the calculator is satisfied by `RpcClient`).
///
/// Only seller-side operations are exposed: match, extract rent, destroy match.
/// Buyer-side operations (create/cancel order) are handled by the frontend.
///
/// The `pending` registry is shared with the REST API so the web console can
/// display the transaction hash in the "waiting for confirmation" modal as soon
/// as the transaction has been broadcasted.
#[derive(Clone)]
pub struct TransactionAssembler {
    rpc: RpcClient,
    fee_rate: u64,
    /// Number of block confirmations to wait for before considering a
    /// transaction final. Shared atomic so the console API can update
    /// it at runtime without a restart.
    confirm_count: Arc<AtomicU64>,
    /// In-memory register of sent-but-unconfirmed transactions, shared with
    /// the `GET /api/console/transactions/pending` endpoint.
    pending: Arc<PendingTxRegistry>,
}

/// Replica of cinnabar `TransactionSkeleton::send_and_wait` confirm loop
/// (skeleton.rs:1208-1240), extracted so we can register the tx hash in
/// `pending` *after* the send but *before* confirmation.
async fn wait_for_confirmation(
    rpc: &RpcClient,
    hash: &H256,
    confirm_count: u8,
    wait_timeout: Duration,
) -> Result<(), AppError> {
    if confirm_count == 0 {
        return Ok(());
    }
    let mut block_number = 0u64;
    let mut time_used = Duration::from_secs(0);
    let interval = Duration::from_secs(3);
    loop {
        if time_used > wait_timeout {
            return Err(AppError::ChainError(format!(
                "Transaction assembly error: timeout waiting tx: {hash:#x}"
            )));
        }
        time_used += interval;
        tokio::time::sleep(interval).await;
        let tx = rpc
            .get_transaction(hash)
            .await
            .map_err(|e| AppError::ChainError(format!("Transaction assembly error: {}", e)))?
            .ok_or_else(|| AppError::ChainError(format!("no tx found: {hash:#x}")))?;
        if tx.tx_status.status == Status::Rejected {
            let reason = tx.tx_status.reason.unwrap_or_else(|| "unknown".to_string());
            return Err(AppError::ChainError(format!(
                "Transaction assembly error: tx {hash:#x} rejected, reason: {reason}"
            )));
        }
        if tx.tx_status.status != Status::Committed {
            continue;
        }
        if block_number == 0 {
            if let Some(number) = tx.tx_status.block_number {
                block_number = number.into();
            }
        } else {
            let tip_number = rpc
                .get_tip_header()
                .await
                .map_err(|e| AppError::ChainError(format!("Transaction assembly error: {}", e)))?
                .inner
                .number;
            if tip_number.value() >= block_number + confirm_count as u64 {
                return Ok(());
            }
        }
    }
}

impl TransactionAssembler {
    /// Create a new transaction assembler.
    pub fn new(
        rpc: RpcClient,
        fee_rate: u64,
        confirm_count: Arc<AtomicU64>,
        pending: Arc<PendingTxRegistry>,
    ) -> Self {
        Self {
            rpc,
            fee_rate,
            confirm_count,
            pending,
        }
    }

    /// Get a reference to the underlying RPC client.
    pub fn rpc(&self) -> &RpcClient {
        &self.rpc
    }

    fn map_err(e: impl std::fmt::Display) -> AppError {
        AppError::ChainError(format!("Transaction assembly error: {}", e))
    }

    // -----------------------------------------------------------------------
    // Private: send, register, wait
    // -----------------------------------------------------------------------

    /// Send a transaction skeleton, register its hash in the pending registry,
    /// then wait for on-chain confirmation.
    ///
    /// On all exit paths (success, rejection, timeout, RPC error) the entry is
    /// removed from the registry so the poll endpoint only ever lists genuinely
    /// in-flight transactions.
    async fn send_registered_and_wait(
        &self,
        skeleton: TransactionSkeleton,
        kind: &str,
        context: &str,
    ) -> Result<String, AppError> {
        // 1. Send only — confirm_count=0 returns right after broadcast
        //    (cinnabar skeleton.rs:1204-1206).
        let hash: H256 = skeleton
            .send_and_wait(&self.rpc, 0, None)
            .await
            .map_err(Self::map_err)?;
        let tx_hash_hex = hex::encode(hash.as_bytes());

        // 2. Register so the frontend can discover the hash while we wait
        self.pending.register(kind, context, &tx_hash_hex);
        tracing::info!(
            kind,
            context,
            tx_hash = %tx_hash_hex,
            "Transaction sent, waiting for confirmation"
        );

        // 3. Wait for confirmation, then unconditionally resolve
        let confirms = self.confirm_count.load(Ordering::Relaxed) as u8;
        let result = wait_for_confirmation(
            &self.rpc,
            &hash,
            confirms,
            Duration::from_secs(CONFIRM_TIMEOUT_SECS),
        )
        .await;
        self.pending.resolve(&tx_hash_hex);
        result?;
        Ok(tx_hash_hex)
    }

    // -----------------------------------------------------------------------
    // Match Order
    // -----------------------------------------------------------------------

    /// Match an order with a Fiber channel on-chain (seller-side).
    pub async fn match_order(
        &self,
        seller_address: &str,
        seller_secret_key: &SecretKey,
        order_info: OrderInfo,
        match_args: MatchArgs,
    ) -> Result<String, AppError> {
        let seller_addr = Address::from_str(seller_address)
            .map_err(|e| AppError::BadRequest(format!("Invalid seller address: {e}")))?;

        // Capture context before order_info is consumed
        let context = hex::encode(order_info.order_outpoint.tx_hash);

        let build_instruction =
            opticrum_calc::match_order(seller_addr.clone(), order_info, match_args);
        let complete = DefaultInstruction::new(vec![Box::new(AddSecp256k1SighashCellDep {})]);
        let balance = balance_and_sign(&seller_addr, *seller_secret_key, self.fee_rate);

        let (tx, _) = TransactionCalculator::new(vec![build_instruction, complete, balance])
            .new_skeleton(&self.rpc)
            .await
            .map_err(Self::map_err)?;

        self.send_registered_and_wait(tx, "match_order", &context)
            .await
    }

    // -----------------------------------------------------------------------
    // Extract Rent
    // -----------------------------------------------------------------------

    /// Extract linearly-vested rent from a Match cell on-chain (seller-side).
    pub async fn extract_rent(
        &self,
        seller_address: &str,
        seller_secret_key: &SecretKey,
        match_info: MatchInfo,
        tip_block: u64,
    ) -> Result<String, AppError> {
        let seller_addr = Address::from_str(seller_address)
            .map_err(|e| AppError::BadRequest(format!("Invalid seller address: {e}")))?;

        // Capture context before match_info is consumed
        let context = format!(
            "{}:{}",
            hex::encode(match_info.match_outpoint.tx_hash),
            match_info.match_outpoint.index
        );

        let build_instruction =
            opticrum_calc::extract_rent(seller_addr.clone(), match_info, tip_block);
        let complete = DefaultInstruction::new(vec![Box::new(AddSecp256k1SighashCellDep {})]);
        let balance = balance_and_sign(&seller_addr, *seller_secret_key, self.fee_rate);
        let (tx, _) = TransactionCalculator::new(vec![build_instruction, complete, balance])
            .new_skeleton(&self.rpc)
            .await
            .map_err(Self::map_err)?;

        self.send_registered_and_wait(tx, "extract_rent", &context)
            .await
    }

    // -----------------------------------------------------------------------
    // Destroy Match
    // -----------------------------------------------------------------------

    /// Destroy an exhausted Match cell on-chain (seller-side).
    pub async fn destroy_match(
        &self,
        claimant_address: &str,
        claimant_secret_key: &SecretKey,
        match_info: MatchInfo,
        tip_block: u64,
    ) -> Result<String, AppError> {
        let claimant_addr = Address::from_str(claimant_address)
            .map_err(|e| AppError::BadRequest(format!("Invalid claimant address: {e}")))?;

        let context = format!(
            "{}:{}",
            hex::encode(match_info.match_outpoint.tx_hash),
            match_info.match_outpoint.index
        );

        let build_instruction =
            opticrum_calc::destroy_match(claimant_addr.clone(), match_info, tip_block);

        let calc = TransactionCalculator::new(vec![build_instruction]);
        let (mut skeleton, _log) = calc.new_skeleton(&self.rpc).await.map_err(Self::map_err)?;

        let sign_instruction =
            balance_and_sign(&claimant_addr, *claimant_secret_key, self.fee_rate);
        let calc = TransactionCalculator::new(vec![sign_instruction]);
        calc.apply_skeleton(&self.rpc, &mut skeleton)
            .await
            .map_err(Self::map_err)?;

        self.send_registered_and_wait(skeleton, "destroy_match", &context)
            .await
    }
}
