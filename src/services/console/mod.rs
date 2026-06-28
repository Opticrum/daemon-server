//! Console services — scheduler observability and gateway aggregation.
//!
//! This module provides:
//! - `scheduler_state`: shared runtime state for scheduler observability
//! - `gateway_service`: unified aggregation hub for the Web Console frontend

pub mod gateway_service;
pub mod scheduler_state;
