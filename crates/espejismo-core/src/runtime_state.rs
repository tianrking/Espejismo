use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

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
    pub bytes_client_to_remote: u64,
    pub bytes_remote_to_client: u64,
    pub last_open_latency_ms: u64,
    pub last_mux_rtt_ms: Option<u64>,
    pub last_error: Option<String>,
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
        self.inner.lock().expect("runtime state lock").clone()
    }

    pub fn set_tunnel_state(&self, state: impl Into<String>) {
        self.inner.lock().expect("runtime state lock").tunnel_state = state.into();
    }

    pub fn record_connect_success(&self) {
        let mut inner = self.inner.lock().expect("runtime state lock");
        inner.tunnel_state = "connected".to_string();
        inner.consecutive_failures = 0;
        inner.tunnel_reconnect_count = inner.tunnel_reconnect_count.saturating_add(1);
    }

    pub fn record_error(&self, error: impl Into<String>) {
        let mut inner = self.inner.lock().expect("runtime state lock");
        inner.tunnel_state = "degraded".to_string();
        inner.consecutive_failures = inner.consecutive_failures.saturating_add(1);
        push_recent_error(&mut inner.recent_errors, error.into());
    }

    pub fn mark_config_applied(&self) {
        let mut inner = self.inner.lock().expect("runtime state lock");
        inner.config_applied_unix_secs = unix_now_secs();
        inner.egress_policy_version = inner.egress_policy_version.saturating_add(1);
    }

    pub fn update_tunnel_lane(&self, lane: TunnelLaneSnapshot) {
        let mut inner = self.inner.lock().expect("runtime state lock");
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
}

fn push_recent_error(errors: &mut Vec<String>, error: String) {
    let mut queue: VecDeque<String> = errors.drain(..).collect();
    queue.push_back(error);
    while queue.len() > 8 {
        queue.pop_front();
    }
    *errors = queue.into();
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
