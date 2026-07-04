//! Shared scheduler state for console observability.
//!
//! Updated by scheduler loops on each cycle, read by the console API
//! to expose runtime scheduler status and structured activity events.

use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

const MAX_EVENTS: usize = 200;

/// A single structured log event for the admin automation console.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SchedulerEvent {
    pub id: u64,
    pub ts_ms: u64,
    pub source: String,
    pub level: String,
    pub message: String,
}

/// Snapshot of a single scheduler component's state.
#[derive(Debug, Clone, Default)]
pub struct CycleState {
    /// Unix timestamp (seconds) of the last completed cycle.
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
    /// Chain cache indexer state.
    pub indexer: CycleState,
    /// Latest known CKB tip block number.
    pub tip_block: u64,
    /// Ring buffer of recent activity events.
    events: Vec<SchedulerEvent>,
    next_event_id: u64,
}

impl SchedulerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn latest_event_id(&self) -> u64 {
        self.next_event_id
    }

    pub fn events_since(&self, since: u64) -> Vec<SchedulerEvent> {
        self.events
            .iter()
            .filter(|e| e.id > since)
            .cloned()
            .collect()
    }
}

/// Thread-safe handle to shared scheduler state.
pub type SharedSchedulerState = Arc<RwLock<SchedulerState>>;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Truncate a hex string for console display (head…tail).
pub fn trunc_hex(value: &str, head: usize, tail: usize) -> String {
    if value.len() <= head + tail + 3 {
        return value.to_string();
    }
    format!("{}…{}", &value[..head], &value[value.len() - tail..])
}

/// Push a structured event when scheduler state is available.
pub fn push_event(
    state: Option<&SharedSchedulerState>,
    source: &str,
    level: &str,
    message: impl Into<String>,
) {
    let Some(state) = state else {
        return;
    };
    if let Ok(mut s) = state.write() {
        s.next_event_id += 1;
        let evt = SchedulerEvent {
            id: s.next_event_id,
            ts_ms: now_ms(),
            source: source.to_string(),
            level: level.to_string(),
            message: message.into(),
        };
        s.events.push(evt);
        if s.events.len() > MAX_EVENTS {
            let drain = s.events.len() - MAX_EVENTS;
            s.events.drain(0..drain);
        }
    }
}

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
        assert!(state.events_since(0).is_empty());
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

    #[test]
    fn push_event_increments_and_filters_since() {
        let shared: SharedSchedulerState = Arc::new(RwLock::new(SchedulerState::new()));
        push_event(Some(&shared), "matcher", "info", "cycle start");
        push_event(Some(&shared), "matcher", "info", "cycle done");
        let s = shared.read().unwrap();
        assert_eq!(s.latest_event_id(), 2);
        assert_eq!(s.events_since(0).len(), 2);
        assert_eq!(s.events_since(1).len(), 1);
        assert_eq!(s.events_since(1)[0].message, "cycle done");
    }

    #[test]
    fn trunc_hex_shortens_long_values() {
        let s = "abcdef0123456789abcdef0123456789";
        assert_eq!(trunc_hex(s, 6, 4), "abcdef…6789");
    }
}
