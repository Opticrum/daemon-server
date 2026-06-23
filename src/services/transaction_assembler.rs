//! Transaction assembler — bridges `opticrum_calculator` Instruction builders
//! with the server's `ChainProvider`, `Signer`, and `RpcClient`.
//!
//! This is the production transaction pipeline. It replaces the placeholder
//! format-string pattern with real CKB transaction assembly, signing, and
//! broadcasting.
//!
//! ## Architecture
//!
//! ```text
//! API handler
//!   → service function (e.g. create_order)
//!     → TransactionAssembler::create_order()
//!       → opticrum_calculator::create_order()  // builds Instruction<T>
//!       → TransactionCalculator::new_skeleton() // builds TransactionSkeleton
//!       → balance_and_sign()                   // adds secp256k1 signatures
//!       → TransactionCalculator::apply_skeleton()
//!       → skeleton.send_and_wait()             // broadcasts to CKB RPC
//!       → return tx_hash
//! ```

use ckb_cinnabar_calculator::{
    address::Address,
    instruction::{predefined::balance_and_sign, TransactionCalculator},
    re_exports::ckb_types::packed::Script,
    rpc::RpcClient,
};
use opticrum_calculator::{
    calculator as opticrum_calc,
    types::{AnnualYield, MatchArgs, MatchInfo, OrderArgs, OrderData, OrderInfo},
};
use secp256k1::SecretKey;
use std::str::FromStr;

use crate::error::AppError;
use crate::services::internal_signer::InternalSigner;

/// High-level transaction assembler that produces real on-chain transactions.
///
/// Created once at startup with a concrete `RpcClient` (the generic `T: RPC`
/// bound required by the calculator is satisfied by `RpcClient`).
pub struct TransactionAssembler {
    rpc: RpcClient,
    fee_rate: u64,
}

impl TransactionAssembler {
    /// Create a new transaction assembler.
    pub fn new(rpc: RpcClient, fee_rate: u64) -> Self {
        Self { rpc, fee_rate }
    }

    /// Get a reference to the underlying RPC client.
    pub fn rpc(&self) -> &RpcClient {
        &self.rpc
    }

    fn map_err(e: impl std::fmt::Display) -> AppError {
        AppError::ChainError(format!("Transaction assembly error: {}", e))
    }

    // -----------------------------------------------------------------------
    // Create Order
    // -----------------------------------------------------------------------

    /// Create a liquidity order on-chain and return the tx_hash.
    ///
    /// Requires the buyer's private key for signing.
    pub async fn create_order(
        &self,
        buyer_address: &str,
        buyer_secret_key: &SecretKey,
        channel_capacity: u64,
        escrow_blocks: u64,
        annual_yield_percent: u8,
        fiber_pubkey: [u8; 32],
        buyer_lock_hash: [u8; 32],
        xudt_type_script: Option<Script>,
    ) -> Result<String, AppError> {
        let buyer_addr = Address::from_str(buyer_address)
            .map_err(|e| AppError::BadRequest(format!("Invalid buyer address: {e}")))?;

        let order_args = OrderArgs::new(fiber_pubkey, buyer_lock_hash);
        let order_data = OrderData::new(0, channel_capacity, escrow_blocks);
        let annual_yield = AnnualYield(annual_yield_percent);

        // Step 1: Build the Opticrum transaction structure
        let build_instruction = opticrum_calc::create_order(
            buyer_addr.clone(),
            &order_args,
            &order_data,
            annual_yield,
            xudt_type_script,
        );

        // Step 2: Build skeleton
        let calc = TransactionCalculator::new(vec![build_instruction]);
        let (mut skeleton, _log) = calc
            .new_skeleton(&self.rpc)
            .await
            .map_err(Self::map_err)?;

        // Step 3: Balance + sign with buyer's key
        let sign_instruction = balance_and_sign(
            &buyer_addr,
            *buyer_secret_key,
            self.fee_rate,
        );
        let calc = TransactionCalculator::new(vec![sign_instruction]);
        calc.apply_skeleton(&self.rpc, &mut skeleton)
            .await
            .map_err(Self::map_err)?;

        // Step 4: Broadcast
        let tx_hash = skeleton
            .send_and_wait(&self.rpc, 0, None)
            .await
            .map_err(Self::map_err)?;

        Ok(hex::encode(tx_hash.as_bytes()))
    }

    // -----------------------------------------------------------------------
    // Cancel Order
    // -----------------------------------------------------------------------

    /// Cancel an unmatched order on-chain.
    pub async fn cancel_order(
        &self,
        buyer_address: &str,
        buyer_secret_key: &SecretKey,
        order_info: OrderInfo,
    ) -> Result<String, AppError> {
        let buyer_addr = Address::from_str(buyer_address)
            .map_err(|e| AppError::BadRequest(format!("Invalid buyer address: {e}")))?;

        let build_instruction = opticrum_calc::cancel_order(buyer_addr.clone(), order_info);

        let calc = TransactionCalculator::new(vec![build_instruction]);
        let (mut skeleton, _log) = calc
            .new_skeleton(&self.rpc)
            .await
            .map_err(Self::map_err)?;

        let sign_instruction = balance_and_sign(&buyer_addr, *buyer_secret_key, self.fee_rate);
        let calc = TransactionCalculator::new(vec![sign_instruction]);
        calc.apply_skeleton(&self.rpc, &mut skeleton)
            .await
            .map_err(Self::map_err)?;

        let tx_hash = skeleton
            .send_and_wait(&self.rpc, 0, None)
            .await
            .map_err(Self::map_err)?;

        Ok(hex::encode(tx_hash.as_bytes()))
    }

    // -----------------------------------------------------------------------
    // Match Order
    // -----------------------------------------------------------------------

    /// Match an order with a Fiber channel on-chain.
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

        let calc = TransactionCalculator::new(vec![build_instruction]);
        let (mut skeleton, _log) = calc
            .new_skeleton(&self.rpc)
            .await
            .map_err(Self::map_err)?;

        let sign_instruction = balance_and_sign(&seller_addr, *seller_secret_key, self.fee_rate);
        let calc = TransactionCalculator::new(vec![sign_instruction]);
        calc.apply_skeleton(&self.rpc, &mut skeleton)
            .await
            .map_err(Self::map_err)?;

        let tx_hash = skeleton
            .send_and_wait(&self.rpc, 0, None)
            .await
            .map_err(Self::map_err)?;

        Ok(hex::encode(tx_hash.as_bytes()))
    }

    // -----------------------------------------------------------------------
    // Extract Rent
    // -----------------------------------------------------------------------

    /// Extract linearly-vested rent from a Match cell on-chain.
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

        let calc = TransactionCalculator::new(vec![build_instruction]);
        let (mut skeleton, _log) = calc
            .new_skeleton(&self.rpc)
            .await
            .map_err(Self::map_err)?;

        let sign_instruction = balance_and_sign(&seller_addr, *seller_secret_key, self.fee_rate);
        let calc = TransactionCalculator::new(vec![sign_instruction]);
        calc.apply_skeleton(&self.rpc, &mut skeleton)
            .await
            .map_err(Self::map_err)?;

        let tx_hash = skeleton
            .send_and_wait(&self.rpc, 0, None)
            .await
            .map_err(Self::map_err)?;

        Ok(hex::encode(tx_hash.as_bytes()))
    }

    // -----------------------------------------------------------------------
    // Destroy Match
    // -----------------------------------------------------------------------

    /// Destroy an exhausted Match cell on-chain.
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
        let (mut skeleton, _log) = calc
            .new_skeleton(&self.rpc)
            .await
            .map_err(Self::map_err)?;

        let sign_instruction =
            balance_and_sign(&claimant_addr, *claimant_secret_key, self.fee_rate);
        let calc = TransactionCalculator::new(vec![sign_instruction]);
        calc.apply_skeleton(&self.rpc, &mut skeleton)
            .await
            .map_err(Self::map_err)?;

        let tx_hash = skeleton
            .send_and_wait(&self.rpc, 0, None)
            .await
            .map_err(Self::map_err)?;

        Ok(hex::encode(tx_hash.as_bytes()))
    }
}

// ---------------------------------------------------------------------------
// Convenience bridge: InternalSigner → TransactionAssembler
// ---------------------------------------------------------------------------

impl TransactionAssembler {
    /// Sign and execute a transaction using an `InternalSigner` (holds the
    /// decrypted `SecretKey` and wallet address).
    ///
    /// This is the recommended path for automated operations (auto-match,
    /// scheduler extraction) that use server-stored keys.
    pub fn sign_with<'a>(&'a self, signer: &'a InternalSigner) -> SignerBridge<'a> {
        SignerBridge {
            assembler: self,
            signer,
        }
    }
}

/// Temporary bridge that pairs a `TransactionAssembler` with an `InternalSigner`
/// for ergonomic tx building + signing + broadcasting.
pub struct SignerBridge<'a> {
    assembler: &'a TransactionAssembler,
    signer: &'a InternalSigner,
}

impl SignerBridge<'_> {
    /// Create an order using the internal signer's key.
    pub async fn create_order(
        &self,
        channel_capacity: u64,
        escrow_blocks: u64,
        annual_yield_percent: u8,
        fiber_pubkey: [u8; 32],
        buyer_lock_hash: [u8; 32],
        xudt_type_script: Option<Script>,
        output_index: &mut i32,
    ) -> Result<String, AppError> {
        let tx_hash = self
            .assembler
            .create_order(
                self.signer.ckb_address(),
                self.signer.secret_key(),
                channel_capacity,
                escrow_blocks,
                annual_yield_percent,
                fiber_pubkey,
                buyer_lock_hash,
                xudt_type_script,
            )
            .await?;
        *output_index = 0; // Order Cell is always output[0]
        Ok(tx_hash)
    }
}
