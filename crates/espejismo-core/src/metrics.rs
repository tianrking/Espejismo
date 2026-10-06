//! Bounded-cardinality counters and snapshots for operational monitoring.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;

// Keep scrape label cardinality and the backing maps bounded even when callers
// provide attacker-controlled user names or failure strings.
const MAX_USER_METRIC_SERIES: usize = 128;
const MAX_FAILURE_REASON_SERIES: usize = 32;
const OTHER_USER: &str = "other";
const OTHER_FAILURE_REASON: &str = "other";

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
    egress_denied: AtomicU64,
    session_rotations: AtomicU64,
    key_updates: AtomicU64,
    bytes_client_to_remote: AtomicU64,
    bytes_remote_to_client: AtomicU64,
    users: Mutex<BTreeMap<String, UserMetricsSnapshot>>,
    stream_failure_reasons: Mutex<BTreeMap<String, u64>>,
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
    pub egress_denied: u64,
    pub session_rotations: u64,
    pub key_updates: u64,
    pub bytes_client_to_remote: u64,
    pub bytes_remote_to_client: u64,
    pub stream_failure_reasons: Vec<ReasonCountSnapshot>,
    pub users: Vec<UserMetricsSnapshot>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ReasonCountSnapshot {
    pub reason: String,
    pub count: u64,
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

    pub fn inc_stream_failed_reason(&self, reason: impl AsRef<str>) {
        self.inc_stream_failed();
        let mut reasons = lock_reason_metrics(&self.inner.stream_failure_reasons);
        let sanitized = sanitize_reason(reason.as_ref());
        let key = if reasons.contains_key(&sanitized)
            || sanitized == OTHER_FAILURE_REASON
            || reasons.len() < MAX_FAILURE_REASON_SERIES - 1
        {
            sanitized
        } else {
            OTHER_FAILURE_REASON.to_string()
        };
        *reasons.entry(key).or_insert(0) += 1;
    }

    pub fn inc_egress_denied(&self) {
        self.inner.egress_denied.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_session_rotation(&self) {
        self.inner.session_rotations.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_key_update(&self) {
        self.inner.key_updates.fetch_add(1, Ordering::Relaxed);
    }

    /// Add payload byte totals in each direction. Per-user byte counters are
    /// updated separately by [`Self::add_user_tunnel_bytes`].
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
        let users = lock_user_metrics(&self.inner.users)
            .values()
            .cloned()
            .collect();
        let stream_failure_reasons = lock_reason_metrics(&self.inner.stream_failure_reasons)
            .iter()
            .map(|(reason, count)| ReasonCountSnapshot {
                reason: reason.clone(),
                count: *count,
            })
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
            egress_denied: self.inner.egress_denied.load(Ordering::Relaxed),
            session_rotations: self.inner.session_rotations.load(Ordering::Relaxed),
            key_updates: self.inner.key_updates.load(Ordering::Relaxed),
            bytes_client_to_remote: self.inner.bytes_client_to_remote.load(Ordering::Relaxed),
            bytes_remote_to_client: self.inner.bytes_remote_to_client.load(Ordering::Relaxed),
            stream_failure_reasons,
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
            "egress_denied_total",
            snapshot.egress_denied,
        );
        metric(
            &mut output,
            role,
            "session_rotations_total",
            snapshot.session_rotations,
        );
        metric(&mut output, role, "key_updates_total", snapshot.key_updates);
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
        for reason in &snapshot.stream_failure_reasons {
            reason_metric(
                &mut output,
                role,
                &reason.reason,
                "stream_failure_reason_total",
                reason.count,
            );
        }
        output
    }

    fn with_user(&self, user: &str, f: impl FnOnce(&mut UserMetricsSnapshot)) {
        let mut users = lock_user_metrics(&self.inner.users);
        let key = if users.contains_key(user)
            || user == OTHER_USER
            || users.len() < MAX_USER_METRIC_SERIES - 1
        {
            user
        } else {
            OTHER_USER
        };
        let entry = users
            .entry(key.to_string())
            .or_insert_with(|| UserMetricsSnapshot {
                user: key.to_string(),
                ..UserMetricsSnapshot::default()
            });
        f(entry);
    }
}

fn lock_user_metrics(
    metrics: &Mutex<BTreeMap<String, UserMetricsSnapshot>>,
) -> MutexGuard<'_, BTreeMap<String, UserMetricsSnapshot>> {
    metrics
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn lock_reason_metrics(
    metrics: &Mutex<BTreeMap<String, u64>>,
) -> MutexGuard<'_, BTreeMap<String, u64>> {
    metrics
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn metric(output: &mut String, role: &str, name: &str, value: u64) {
    output.push_str("espejismo_");
    output.push_str(name);
    output.push_str("{role=\"");
    push_label_value(output, role);
    output.push_str("\"} ");
    output.push_str(&value.to_string());
    output.push('\n');
}

fn user_metric(output: &mut String, role: &str, user: &str, name: &str, value: u64) {
    output.push_str("espejismo_");
    output.push_str(name);
    output.push_str("{role=\"");
    push_label_value(output, role);
    output.push_str("\",user=\"");
    push_label_value(output, user);
    output.push_str("\"} ");
    output.push_str(&value.to_string());
    output.push('\n');
}

fn reason_metric(output: &mut String, role: &str, reason: &str, name: &str, value: u64) {
    output.push_str("espejismo_");
    output.push_str(name);
    output.push_str("{role=\"");
    push_label_value(output, role);
    output.push_str("\",reason=\"");
    push_label_value(output, reason);
    output.push_str("\"} ");
    output.push_str(&value.to_string());
    output.push('\n');
}

// Prometheus text format requires backslash, quote, and line-feed escaping in
// label values. Escape every label through the same path so unusual role names
// and user supplied labels cannot split a sample into malformed exposition.
fn push_label_value(output: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            _ => output.push(ch),
        }
    }
}

fn sanitize_reason(reason: &str) -> String {
    reason
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' => ch,
            _ => '_',
        })
        .take(64)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prometheus_label_value_escapes_backslash_quote_and_newline() {
        let mut escaped = String::new();
        push_label_value(&mut escaped, "slash\\ quote\" line\nnext");
        assert_eq!(escaped, "slash\\\\ quote\\\" line\\nnext");
    }

    #[test]
    fn prometheus_metrics_escape_role_and_user_labels() {
        let metrics = Metrics::default();
        metrics.inc_user_handshake_success("alice\"\\\nremote");
        let rendered = metrics.render_prometheus("local\"\\\nrole");
        assert!(rendered.contains("role=\"local\\\"\\\\\\nrole\""));
        assert!(rendered.contains("user=\"alice\\\"\\\\\\nremote\""));
        assert!(!rendered.contains("\nremote\""));
    }

    #[test]
    fn prometheus_reason_label_uses_sanitized_reason() {
        let metrics = Metrics::default();
        metrics.inc_stream_failed_reason("bad\"\\\nreason");

        let rendered = metrics.render_prometheus("server");
        assert!(rendered.contains(
            "espejismo_stream_failure_reason_total{role=\"server\",reason=\"bad___reason\"} 1\n"
        ));
        assert!(!rendered.contains("reason=\"bad\""));
    }

    #[test]
    fn user_metric_series_are_bounded_with_overflow_bucket() {
        let metrics = Metrics::default();
        for index in 0..(MAX_USER_METRIC_SERIES + 20) {
            let user = format!("user-{index}");
            metrics.inc_user_handshake_success(&user);
            metrics.inc_user_stream_opened(&user);
            metrics.add_user_tunnel_bytes(&user, index as u64 + 1, (index as u64 + 1) * 10);
        }

        let snapshot = metrics.snapshot("server");
        assert_eq!(snapshot.users.len(), MAX_USER_METRIC_SERIES);
        let overflow = snapshot
            .users
            .iter()
            .find(|user| user.user == OTHER_USER)
            .unwrap();
        assert_eq!(overflow.handshake_success, 21);
        assert_eq!(overflow.stream_opened, 21);
        // The first 127 distinct users keep their own series; all later users
        // share `other`, including both directional byte counters.
        assert_eq!(overflow.bytes_client_to_remote, (128_u64..=148).sum::<u64>());
        assert_eq!(
            overflow.bytes_remote_to_client,
            (128_u64..=148).map(|n| n * 10).sum::<u64>()
        );

        let rendered = metrics.render_prometheus("remote");
        for metric_name in [
            "user_handshake_success_total",
            "user_stream_opened_total",
            "user_bytes_client_to_remote_total",
            "user_bytes_remote_to_client_total",
        ] {
            assert_eq!(
                rendered
                    .lines()
                    .filter(|line| line.starts_with(&format!("espejismo_{metric_name}{{")))
                    .count(),
                MAX_USER_METRIC_SERIES,
                "unexpected series count for {metric_name}"
            );
        }
        assert!(rendered.contains(
            "espejismo_user_bytes_client_to_remote_total{role=\"remote\",user=\"other\"} 2898\n"
        ));
        assert!(rendered.contains(
            "espejismo_user_bytes_remote_to_client_total{role=\"remote\",user=\"other\"} 28980\n"
        ));
    }

    #[test]
    fn failure_reason_series_are_bounded_with_overflow_bucket() {
        let metrics = Metrics::default();
        for index in 0..(MAX_FAILURE_REASON_SERIES + 20) {
            metrics.inc_stream_failed_reason(format!("reason-{index}"));
        }

        let snapshot = metrics.snapshot("server");
        assert_eq!(
            snapshot.stream_failure_reasons.len(),
            MAX_FAILURE_REASON_SERIES
        );
        let overflow = snapshot
            .stream_failure_reasons
            .iter()
            .find(|reason| reason.reason == OTHER_FAILURE_REASON)
            .unwrap();
        assert_eq!(overflow.count, 21);
        assert_eq!(
            snapshot.stream_failed,
            (MAX_FAILURE_REASON_SERIES + 20) as u64
        );
    }

    #[test]
    fn byte_counters_accumulate_independently_and_match_user_totals() {
        let metrics = Metrics::default();
        metrics.add_tunnel_bytes(7, 13);
        metrics.add_tunnel_bytes(5, 0);
        metrics.add_user_tunnel_bytes("alice", 7, 13);
        metrics.add_user_tunnel_bytes("alice", 5, 0);
        metrics.add_user_tunnel_bytes("bob", 0, 9);

        let snapshot = metrics.snapshot("server");
        assert_eq!(snapshot.bytes_client_to_remote, 12);
        assert_eq!(snapshot.bytes_remote_to_client, 13);
        let alice = snapshot.users.iter().find(|user| user.user == "alice").unwrap();
        assert_eq!(alice.bytes_client_to_remote, 12);
        assert_eq!(alice.bytes_remote_to_client, 13);
        let bob = snapshot.users.iter().find(|user| user.user == "bob").unwrap();
        assert_eq!(bob.bytes_client_to_remote, 0);
        assert_eq!(bob.bytes_remote_to_client, 9);
    }

    #[test]
    fn cloned_metrics_share_counter_updates() {
        let metrics = Metrics::default();
        let clone = metrics.clone();
        clone.inc_accepted();
        clone.add_tunnel_bytes(3, 11);

        let snapshot = metrics.snapshot("client");
        assert_eq!(snapshot.accepted_connections, 1);
        assert_eq!(snapshot.bytes_client_to_remote, 3);
        assert_eq!(snapshot.bytes_remote_to_client, 11);
    }
}
