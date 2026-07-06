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
//!       → skeleton.send_and_wait()             // broadcasts to CKB RPC
//!       → return tx_hash
//! ```

use ckb_cinnabar_calculator::{
    address::Address,
    instruction::{predefined::balance_and_sign, DefaultInstruction, TransactionCalculator},
    operation::basic::AddSecp256k1SighashCellDep,
    rpc::RpcClient,
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
#[derive(Clone)]
pub struct TransactionAssembler {
    rpc: RpcClient,
    fee_rate: u64,
    /// Number of block confirmations to wait for before considering a
    /// transaction final. Shared atomic so the console API can update
    /// it at runtime without a restart.
    confirm_count: Arc<AtomicU64>,
}

impl TransactionAssembler {
    /// Create a new transaction assembler.
    pub fn new(rpc: RpcClient, fee_rate: u64, confirm_count: Arc<AtomicU64>) -> Self {
        Self {
            rpc,
            fee_rate,
            confirm_count,
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

        let build_instruction =
            opticrum_calc::match_order(seller_addr.clone(), order_info, match_args);
        let complete = DefaultInstruction::new(vec![Box::new(AddSecp256k1SighashCellDep {})]);
        let balance = balance_and_sign(&seller_addr, *seller_secret_key, self.fee_rate);

        // For debug
        let (tx, _) = TransactionCalculator::new(vec![build_instruction, complete, balance])
            .new_skeleton(&self.rpc)
            .await
            .map_err(Self::map_err)?;

        let tx_hash = tx
            .send_and_wait(
                &self.rpc,
                self.confirm_count.load(Ordering::Relaxed) as u8,
                Some(Duration::from_secs(CONFIRM_TIMEOUT_SECS)),
            )
            .await
            .map_err(Self::map_err)?;

        Ok(hex::encode(tx_hash.as_bytes()))
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

        let build_instruction =
            opticrum_calc::extract_rent(seller_addr.clone(), match_info, tip_block);
        let complete = DefaultInstruction::new(vec![Box::new(AddSecp256k1SighashCellDep {})]);
        let balance = balance_and_sign(&seller_addr, *seller_secret_key, self.fee_rate);
        let (tx, _) = TransactionCalculator::new(vec![build_instruction, complete, balance])
            .new_skeleton(&self.rpc)
            .await
            .map_err(Self::map_err)?;

        let tx_hash = tx
            .send_and_wait(
                &self.rpc,
                self.confirm_count.load(Ordering::Relaxed) as u8,
                Some(Duration::from_secs(CONFIRM_TIMEOUT_SECS)),
            )
            .await
            .map_err(Self::map_err)?;

        Ok(hex::encode(tx_hash.as_bytes()))
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

        let tx_hash = skeleton
            .send_and_wait(
                &self.rpc,
                self.confirm_count.load(Ordering::Relaxed) as u8,
                Some(Duration::from_secs(CONFIRM_TIMEOUT_SECS)),
            )
            .await
            .map_err(Self::map_err)?;

        Ok(hex::encode(tx_hash.as_bytes()))
    }
}
