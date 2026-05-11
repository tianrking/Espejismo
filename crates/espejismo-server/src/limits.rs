use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use tokio::sync::Mutex;
use tokio::time::{sleep_until, Instant};

#[derive(Clone, Debug, Default)]
pub struct UserLimitConfig {
    pub quota_bytes: Option<u64>,
    pub quota_window: Duration,
    pub bandwidth_bytes_per_sec: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct UserLimitRegistry {
    inner: Arc<Mutex<HashMap<String, UserLimitState>>>,
}

#[derive(Clone, Debug)]
struct UserLimitState {
    config: UserLimitConfig,
    window_started: Instant,
    used_bytes: u64,
    next_send: Instant,
}

impl UserLimitRegistry {
    pub fn new(configs: impl IntoIterator<Item = (String, UserLimitConfig)>) -> Self {
        let now = Instant::now();
        let states = configs
            .into_iter()
            .map(|(user, config)| {
                (
                    user,
                    UserLimitState {
                        config,
                        window_started: now,
                        used_bytes: 0,
                        next_send: now,
                    },
                )
            })
            .collect();
        Self {
            inner: Arc::new(Mutex::new(states)),
        }
    }

    pub async fn ensure_open(&self, user: &str) -> Result<()> {
        let mut states = self.inner.lock().await;
        let state = state_for_user(&mut states, user);
        refresh_window(state);
        if let Some(limit) = state.config.quota_bytes {
            if state.used_bytes >= limit {
                bail!("user quota exceeded");
            }
        }
        Ok(())
    }

    pub async fn account_and_throttle(&self, user: &str, bytes: u64) -> Result<()> {
        if bytes == 0 {
            return Ok(());
        }

        let sleep_until_instant = {
            let mut states = self.inner.lock().await;
            let state = state_for_user(&mut states, user);
            refresh_window(state);
            if let Some(limit) = state.config.quota_bytes {
                let next = state.used_bytes.saturating_add(bytes);
                if next > limit {
                    bail!("user quota exceeded");
                }
                state.used_bytes = next;
            }

            state.config.bandwidth_bytes_per_sec.map(|rate| {
                let now = Instant::now();
                if state.next_send < now {
                    state.next_send = now;
                }
                let wake = state.next_send;
                let delay = Duration::from_secs_f64(bytes as f64 / rate as f64);
                state.next_send += delay;
                wake
            })
        };

        if let Some(wake) = sleep_until_instant {
            let now = Instant::now();
            if wake > now {
                sleep_until(wake).await;
            }
        }

        Ok(())
    }
}

fn state_for_user<'a>(
    states: &'a mut HashMap<String, UserLimitState>,
    user: &str,
) -> &'a mut UserLimitState {
    states
        .entry(user.to_string())
        .or_insert_with(|| UserLimitState {
            config: UserLimitConfig::default(),
            window_started: Instant::now(),
            used_bytes: 0,
            next_send: Instant::now(),
        })
}

fn refresh_window(state: &mut UserLimitState) {
    let now = Instant::now();
    if state.config.quota_bytes.is_some()
        && !state.config.quota_window.is_zero()
        && now.duration_since(state.window_started) >= state.config.quota_window
    {
        state.window_started = now;
        state.used_bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn quota_rejects_bytes_over_window_limit() {
        let limits = UserLimitRegistry::new([(
            "alice".to_string(),
            UserLimitConfig {
                quota_bytes: Some(10),
                quota_window: Duration::from_secs(60),
                bandwidth_bytes_per_sec: None,
            },
        )]);

        limits.account_and_throttle("alice", 6).await.unwrap();
        assert!(limits.account_and_throttle("alice", 5).await.is_err());
        limits.account_and_throttle("bob", 1024).await.unwrap();
    }

    #[tokio::test]
    async fn bandwidth_without_backlog_allows_first_chunk() {
        let limits = UserLimitRegistry::new([(
            "alice".to_string(),
            UserLimitConfig {
                quota_bytes: None,
                quota_window: Duration::from_secs(60),
                bandwidth_bytes_per_sec: Some(1024 * 1024),
            },
        )]);

        limits.account_and_throttle("alice", 10).await.unwrap();
    }
}
