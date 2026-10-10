//! Thread-safe snapshots of tunnel lifecycle and lane health.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

const MAX_RECENT_ERRORS: usize = 8;

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeStateSnapshot {
    pub started_at_unix_secs: u64,
    pub config_applied_unix_secs: u64,
    pub tunnel_state: String,
    pub tunnel_reconnect_count: u64,
    pub consecutive_failures: u64,
    pub recent_errors: Vec<String>,
    pub egress_policy_version: u64,
    pub tunnel_lanes: Vec<TunnelLaneSnapshot>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct TunnelLaneSnapshot {
    pub id: usize,
    pub lane: String,
    pub state: String,
    pub reconnect_count: u64,
    pub active_streams: u64,
    pub pending_stream_opens: u64,
    pub streams_opened: u64,
    pub stream_open_failures: u64,
    pub bytes_client_to_remote: u64,
    pub bytes_remote_to_client: u64,
    pub recent_client_to_remote_bps: u64,
    pub recent_remote_to_client_bps: u64,
    pub adaptive_score: u64,
    pub last_open_latency_ms: u64,
    pub last_mux_rtt_ms: Option<u64>,
    pub mux_rtt_trend_ms: Vec<u64>,
    pub session_age_secs: Option<u64>,
    pub last_activity_unix_secs: Option<u64>,
    pub last_error: Option<String>,
    pub last_error_unix_secs: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct RuntimeState {
    inner: Arc<Mutex<RuntimeStateSnapshot>>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        let now = unix_now_secs();
        Self {
            inner: Arc::new(Mutex::new(RuntimeStateSnapshot {
                started_at_unix_secs: now,
                config_applied_unix_secs: now,
                tunnel_state: "starting".to_string(),
                tunnel_reconnect_count: 0,
                consecutive_failures: 0,
                recent_errors: Vec::new(),
                egress_policy_version: 1,
                tunnel_lanes: Vec::new(),
            })),
        }
    }
}

impl RuntimeState {
    pub fn snapshot(&self) -> RuntimeStateSnapshot {
        lock_runtime_state(&self.inner).clone()
    }

    pub fn set_tunnel_state(&self, state: impl Into<String>) {
        lock_runtime_state(&self.inner).tunnel_state = state.into();
    }

    pub fn record_connect_success(&self) {
        let mut inner = lock_runtime_state(&self.inner);
        inner.tunnel_state = "connected".to_string();
        inner.consecutive_failures = 0;
        inner.tunnel_reconnect_count = inner.tunnel_reconnect_count.saturating_add(1);
    }

    pub fn record_error(&self, error: impl Into<String>) {
        let mut inner = lock_runtime_state(&self.inner);
        inner.tunnel_state = "degraded".to_string();
        inner.consecutive_failures = inner.consecutive_failures.saturating_add(1);
        push_recent_error(&mut inner.recent_errors, error.into());
    }

    pub fn mark_config_applied(&self) {
        let mut inner = lock_runtime_state(&self.inner);
        inner.config_applied_unix_secs = unix_now_secs();
        inner.egress_policy_version = inner.egress_policy_version.saturating_add(1);
    }

    pub fn update_tunnel_lane(&self, lane: TunnelLaneSnapshot) {
        let mut inner = lock_runtime_state(&self.inner);
        if let Some(existing) = inner
            .tunnel_lanes
            .iter_mut()
            .find(|existing| existing.id == lane.id)
        {
            *existing = lane;
        } else {
            inner.tunnel_lanes.push(lane);
            inner.tunnel_lanes.sort_by_key(|lane| lane.id);
        }
    }

    pub fn add_tunnel_lane_bytes(
        &self,
        lane_id: usize,
        client_to_remote: u64,
        remote_to_client: u64,
    ) {
        let mut inner = lock_runtime_state(&self.inner);
        if let Some(existing) = inner
            .tunnel_lanes
            .iter_mut()
            .find(|existing| existing.id == lane_id)
        {
            existing.bytes_client_to_remote = existing
                .bytes_client_to_remote
                .saturating_add(client_to_remote);
            existing.bytes_remote_to_client = existing
                .bytes_remote_to_client
                .saturating_add(remote_to_client);
            existing.last_activity_unix_secs = Some(unix_now_secs());
        }
    }
}

fn lock_runtime_state(state: &Mutex<RuntimeStateSnapshot>) -> MutexGuard<'_, RuntimeStateSnapshot> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn push_recent_error(errors: &mut Vec<String>, error: String) {
    // Keep the snapshot's public Vec representation without rebuilding a queue
    // on every error; the bounded shift is cheaper and retains existing semantics.
    if errors.len() == MAX_RECENT_ERRORS {
        errors.remove(0);
    }
    errors.push(error);
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{RuntimeState, TunnelLaneSnapshot};

    #[test]
    fn lane_byte_updates_are_incremental() {
        let state = RuntimeState::default();
        state.update_tunnel_lane(TunnelLaneSnapshot {
            id: 7,
            lane: "bulk".to_string(),
            state: "connected".to_string(),
            bytes_client_to_remote: 10,
            bytes_remote_to_client: 20,
            ..TunnelLaneSnapshot::default()
        });

        state.add_tunnel_lane_bytes(7, 5, 9);

        let lane = state
            .snapshot()
            .tunnel_lanes
            .into_iter()
            .find(|lane| lane.id == 7)
            .unwrap();
        assert_eq!(lane.bytes_client_to_remote, 15);
        assert_eq!(lane.bytes_remote_to_client, 29);
        assert!(lane.last_activity_unix_secs.is_some());
    }

    #[test]
    fn lane_byte_updates_ignore_unknown_lanes() {
        let state = RuntimeState::default();
        state.add_tunnel_lane_bytes(99, 5, 9);
        assert!(state.snapshot().tunnel_lanes.is_empty());
    }

    #[test]
    fn connection_and_failure_transitions_update_counters_consistently() {
        let state = RuntimeState::default();
        state.record_error("first");
        state.record_error("second");
        state.record_connect_success();

        let snapshot = state.snapshot();
        assert_eq!(snapshot.tunnel_state, "connected");
        assert_eq!(snapshot.consecutive_failures, 0);
        assert_eq!(snapshot.tunnel_reconnect_count, 1);
        assert_eq!(snapshot.recent_errors, ["first", "second"]);
    }

    #[test]
    fn recent_errors_keep_only_the_newest_eight_in_order() {
        let state = RuntimeState::default();
        for index in 0..10 {
            state.record_error(format!("error-{index}"));
        }

        let snapshot = state.snapshot();
        assert_eq!(snapshot.tunnel_state, "degraded");
        assert_eq!(snapshot.consecutive_failures, 10);
        assert_eq!(snapshot.recent_errors.len(), 8);
        assert_eq!(snapshot.recent_errors.first().unwrap(), "error-2");
        assert_eq!(snapshot.recent_errors.last().unwrap(), "error-9");
    }

    #[test]
    fn concurrent_updates_are_not_lost() {
        let state = RuntimeState::default();
        let mut workers = Vec::new();
        for _ in 0..8 {
            let state = state.clone();
            workers.push(std::thread::spawn(move || {
                for _ in 0..250 {
                    state.record_connect_success();
                    state.record_error("parallel");
                }
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }

        let snapshot = state.snapshot();
        assert_eq!(snapshot.tunnel_reconnect_count, 2_000);
        assert_eq!(snapshot.consecutive_failures, 1);
        assert_eq!(snapshot.recent_errors.len(), 8);
        assert!(snapshot.recent_errors.iter().all(|error| error == "parallel"));
    }

    #[test]
    fn counters_saturate_instead_of_wrapping() {
        let state = RuntimeState::default();
        {
            let mut snapshot = super::lock_runtime_state(&state.inner);
            snapshot.tunnel_reconnect_count = u64::MAX;
        }
        state.record_connect_success();
        {
            let mut snapshot = super::lock_runtime_state(&state.inner);
            snapshot.consecutive_failures = u64::MAX;
        }
        state.record_error("overflow boundary");

        let snapshot = state.snapshot();
        assert_eq!(snapshot.tunnel_reconnect_count, u64::MAX);
        assert_eq!(snapshot.consecutive_failures, u64::MAX);
    }
}
