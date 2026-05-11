use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    inner: Arc<MetricsInner>,
}

#[derive(Debug, Default)]
struct MetricsInner {
    active_physical_connections: AtomicU64,
    active_streams: AtomicU64,
    accepted_connections: AtomicU64,
    handshake_success: AtomicU64,
    handshake_failure: AtomicU64,
    stream_opened: AtomicU64,
    stream_failed: AtomicU64,
    bytes_client_to_remote: AtomicU64,
    bytes_remote_to_client: AtomicU64,
    users: Mutex<BTreeMap<String, UserMetricsSnapshot>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MetricsSnapshot {
    pub role: String,
    pub active_physical_connections: u64,
    pub active_streams: u64,
    pub accepted_connections: u64,
    pub handshake_success: u64,
    pub handshake_failure: u64,
    pub stream_opened: u64,
    pub stream_failed: u64,
    pub bytes_client_to_remote: u64,
    pub bytes_remote_to_client: u64,
    pub users: Vec<UserMetricsSnapshot>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct UserMetricsSnapshot {
    pub user: String,
    pub handshake_success: u64,
    pub stream_opened: u64,
    pub bytes_client_to_remote: u64,
    pub bytes_remote_to_client: u64,
}

impl Metrics {
    pub fn inc_active_physical(&self) {
        self.inner
            .active_physical_connections
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_active_physical(&self) {
        self.inner
            .active_physical_connections
            .fetch_sub(1, Ordering::Relaxed);
    }

    pub fn inc_active_stream(&self) {
        self.inner.active_streams.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_active_stream(&self) {
        self.inner.active_streams.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn inc_accepted(&self) {
        self.inner
            .accepted_connections
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_handshake_success(&self) {
        self.inner.handshake_success.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_handshake_failure(&self) {
        self.inner.handshake_failure.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_stream_opened(&self) {
        self.inner.stream_opened.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_stream_failed(&self) {
        self.inner.stream_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_tunnel_bytes(&self, client_to_remote: u64, remote_to_client: u64) {
        self.inner
            .bytes_client_to_remote
            .fetch_add(client_to_remote, Ordering::Relaxed);
        self.inner
            .bytes_remote_to_client
            .fetch_add(remote_to_client, Ordering::Relaxed);
    }

    pub fn inc_user_handshake_success(&self, user: &str) {
        self.with_user(user, |entry| {
            entry.handshake_success += 1;
        });
    }

    pub fn inc_user_stream_opened(&self, user: &str) {
        self.with_user(user, |entry| {
            entry.stream_opened += 1;
        });
    }

    pub fn add_user_tunnel_bytes(&self, user: &str, client_to_remote: u64, remote_to_client: u64) {
        self.with_user(user, |entry| {
            entry.bytes_client_to_remote += client_to_remote;
            entry.bytes_remote_to_client += remote_to_client;
        });
    }

    pub fn snapshot(&self, role: impl Into<String>) -> MetricsSnapshot {
        let users = self
            .inner
            .users
            .lock()
            .expect("user metrics mutex poisoned")
            .values()
            .cloned()
            .collect();
        MetricsSnapshot {
            role: role.into(),
            active_physical_connections: self
                .inner
                .active_physical_connections
                .load(Ordering::Relaxed),
            active_streams: self.inner.active_streams.load(Ordering::Relaxed),
            accepted_connections: self.inner.accepted_connections.load(Ordering::Relaxed),
            handshake_success: self.inner.handshake_success.load(Ordering::Relaxed),
            handshake_failure: self.inner.handshake_failure.load(Ordering::Relaxed),
            stream_opened: self.inner.stream_opened.load(Ordering::Relaxed),
            stream_failed: self.inner.stream_failed.load(Ordering::Relaxed),
            bytes_client_to_remote: self.inner.bytes_client_to_remote.load(Ordering::Relaxed),
            bytes_remote_to_client: self.inner.bytes_remote_to_client.load(Ordering::Relaxed),
            users,
        }
    }

    pub fn render_prometheus(&self, role: &str) -> String {
        let snapshot = self.snapshot(role);
        let mut output = String::new();
        metric(
            &mut output,
            role,
            "active_physical_connections",
            snapshot.active_physical_connections,
        );
        metric(&mut output, role, "active_streams", snapshot.active_streams);
        metric(
            &mut output,
            role,
            "accepted_connections_total",
            snapshot.accepted_connections,
        );
        metric(
            &mut output,
            role,
            "handshake_success_total",
            snapshot.handshake_success,
        );
        metric(
            &mut output,
            role,
            "handshake_failure_total",
            snapshot.handshake_failure,
        );
        metric(
            &mut output,
            role,
            "stream_opened_total",
            snapshot.stream_opened,
        );
        metric(
            &mut output,
            role,
            "stream_failed_total",
            snapshot.stream_failed,
        );
        metric(
            &mut output,
            role,
            "bytes_client_to_remote_total",
            snapshot.bytes_client_to_remote,
        );
        metric(
            &mut output,
            role,
            "bytes_remote_to_client_total",
            snapshot.bytes_remote_to_client,
        );
        for user in &snapshot.users {
            user_metric(
                &mut output,
                role,
                &user.user,
                "user_handshake_success_total",
                user.handshake_success,
            );
            user_metric(
                &mut output,
                role,
                &user.user,
                "user_stream_opened_total",
                user.stream_opened,
            );
            user_metric(
                &mut output,
                role,
                &user.user,
                "user_bytes_client_to_remote_total",
                user.bytes_client_to_remote,
            );
            user_metric(
                &mut output,
                role,
                &user.user,
                "user_bytes_remote_to_client_total",
                user.bytes_remote_to_client,
            );
        }
        output
    }

    fn with_user(&self, user: &str, f: impl FnOnce(&mut UserMetricsSnapshot)) {
        let mut users = self
            .inner
            .users
            .lock()
            .expect("user metrics mutex poisoned");
        let entry = users
            .entry(user.to_string())
            .or_insert_with(|| UserMetricsSnapshot {
                user: user.to_string(),
                ..UserMetricsSnapshot::default()
            });
        f(entry);
    }
}

fn metric(output: &mut String, role: &str, name: &str, value: u64) {
    output.push_str("espejismo_");
    output.push_str(name);
    output.push_str("{role=\"");
    output.push_str(role);
    output.push_str("\"} ");
    output.push_str(&value.to_string());
    output.push('\n');
}

fn user_metric(output: &mut String, role: &str, user: &str, name: &str, value: u64) {
    output.push_str("espejismo_");
    output.push_str(name);
    output.push_str("{role=\"");
    output.push_str(role);
    output.push_str("\",user=\"");
    output.push_str(&user.replace('"', "\\\""));
    output.push_str("\"} ");
    output.push_str(&value.to_string());
    output.push('\n');
}
