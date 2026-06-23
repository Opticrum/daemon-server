//! Application error types.
//!
//! `AppError` is the unified error type for the entire server. It implements
//! actix-web's `ResponseError` so handlers can use `?` to propagate errors
//! and get correct HTTP status codes automatically.

use actix_web::{HttpResponse, ResponseError};
use std::fmt;
use tracing::error;

/// Unified application error.
#[derive(Debug)]
pub enum AppError {
    /// Something was not found (404)
    NotFound(String),
    /// Invalid input from the client (400)
    BadRequest(String),
    /// Wallet/key related error (400)
    WalletError(String),
    /// Chain interaction failure (502)
    ChainError(String),
    /// Internal server error (500)
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            Self::WalletError(msg) => write!(f, "Wallet error: {}", msg),
            Self::ChainError(msg) => write!(f, "Chain error: {}", msg),
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let (status, kind) = match self {
            Self::NotFound(_) => (actix_web::http::StatusCode::NOT_FOUND, "not_found"),
            Self::BadRequest(_) => (actix_web::http::StatusCode::BAD_REQUEST, "bad_request"),
            Self::WalletError(_) => (actix_web::http::StatusCode::BAD_REQUEST, "wallet_error"),
            Self::ChainError(_) => (actix_web::http::StatusCode::BAD_GATEWAY, "chain_error"),
            Self::Internal(_) => (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
            ),
        };

        // Log every error response for server monitoring
        error!(
            status = status.as_u16(),
            kind = kind,
            message = %self,
            "Request failed"
        );

        HttpResponse::build(status).json(serde_json::json!({
            "error": kind,
            "message": self.to_string(),
        }))
    }
}

// Allow converting common error types into AppError

impl From<diesel::result::Error> for AppError {
    fn from(e: diesel::result::Error) -> Self {
        Self::Internal(format!("Database error: {e}"))
    }
}

impl From<diesel::r2d2::PoolError> for AppError {
    fn from(e: diesel::r2d2::PoolError) -> Self {
        Self::Internal(format!("Connection pool error: {e}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::BadRequest(format!("JSON error: {}", e))
    }
}

impl From<hex::FromHexError> for AppError {
    fn from(e: hex::FromHexError) -> Self {
        Self::BadRequest(format!("Hex decode error: {}", e))
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Internal(format!("IO error: {}", e))
    }
}
