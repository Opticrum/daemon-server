//! Shared scheduler state for console observability.
//!
//! Updated by scheduler loops on each cycle, read by the console API
//! to expose runtime scheduler status.

use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

/// Snapshot of a single scheduler component's state.
#[derive(Debug, Clone, Default)]
pub struct CycleState {
    /// ISO-8601 timestamp of the last completed cycle.
    pub last_run: Option<String>,
    /// Duration of the last cycle in milliseconds.
    pub last_duration_ms: u64,
    /// Total cycles completed since server start.
    pub cycles: u64,
    /// Total amount processed across all cycles.
    pub total_processed: u64,
    /// Amount processed in the last cycle.
    pub last_processed: u64,
    /// Last error message, if any.
    pub last_error: Option<String>,
}

/// Aggregated scheduler state shared between scheduler loops and the console API.
#[derive(Debug, Clone, Default)]
pub struct SchedulerState {
    /// Rent extractor state.
    pub extractor: CycleState,
    /// Auto-matcher state.
    pub matcher: CycleState,
    /// Latest known CKB tip block number.
    pub tip_block: u64,
}

impl SchedulerState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Thread-safe handle to shared scheduler state.
pub type SharedSchedulerState = Arc<RwLock<SchedulerState>>;

/// Helper: record a successful cycle.
pub fn record_success(
    state: &RwLock<SchedulerState>,
    field: fn(&mut SchedulerState) -> &mut CycleState,
    duration: Duration,
    processed: u64,
) {
    if let Ok(mut s) = state.write() {
        let cs = field(&mut s);
        cs.last_run = Some(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs().to_string())
                .unwrap_or_default(),
        );
        cs.last_duration_ms = duration.as_millis() as u64;
        cs.cycles += 1;
        cs.total_processed += processed;
        cs.last_processed = processed;
        cs.last_error = None;
    }
}

/// Helper: record a failed cycle.
pub fn record_error(
    state: &RwLock<SchedulerState>,
    field: fn(&mut SchedulerState) -> &mut CycleState,
    error: &str,
) {
    if let Ok(mut s) = state.write() {
        let cs = field(&mut s);
        cs.last_run = Some(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs().to_string())
                .unwrap_or_default(),
        );
        cs.last_error = Some(error.to_string());
    }
}

/// Helper: update tip block.
pub fn set_tip_block(state: &RwLock<SchedulerState>, tip: u64) {
    if let Ok(mut s) = state.write() {
        s.tip_block = tip;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn new_state_has_defaults() {
        let state = SchedulerState::new();
        assert!(state.extractor.last_run.is_none());
        assert_eq!(state.extractor.cycles, 0);
        assert_eq!(state.extractor.total_processed, 0);
        assert_eq!(state.tip_block, 0);
    }

    #[test]
    fn record_success_updates_extractor() {
        let state = RwLock::new(SchedulerState::new());
        record_success(&state, |s| &mut s.extractor, Duration::from_secs(2), 5000);
        let s = state.read().unwrap();
        assert_eq!(s.extractor.cycles, 1);
        assert_eq!(s.extractor.total_processed, 5000);
        assert_eq!(s.extractor.last_processed, 5000);
        assert!(s.extractor.last_duration_ms > 0);
        assert!(s.extractor.last_run.is_some());
        assert!(s.extractor.last_error.is_none());
    }

    #[test]
    fn record_error_preserves_message() {
        let state = RwLock::new(SchedulerState::new());
        record_error(&state, |s| &mut s.matcher, "chain timeout");
        let s = state.read().unwrap();
        assert_eq!(s.matcher.last_error.as_deref(), Some("chain timeout"));
        assert!(s.matcher.last_run.is_some());
    }

    #[test]
    fn set_tip_block_persists() {
        let state = RwLock::new(SchedulerState::new());
        set_tip_block(&state, 12345);
        assert_eq!(state.read().unwrap().tip_block, 12345);
    }
}
