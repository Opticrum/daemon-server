//! Opticrum Rust Server library — provides the HTTP API, services, and DB layer.
//!
//! This crate can be used as a library (for tests and integration) or
//! as a binary (via `main.rs`).

pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod scheduler;
pub mod services;
