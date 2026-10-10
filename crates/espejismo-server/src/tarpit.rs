use std::collections::VecDeque;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{Instant, interval};
use tracing::debug;

const MAX_SWEEP_INTERVAL: Duration = Duration::from_secs(5);

fn insert_bounded<T>(
    entries: &mut VecDeque<(Instant, T)>,
    capacity: usize,
    expires_at: Instant,
    value: T,
) -> bool {
    if capacity == 0 {
        return false;
    }
    while entries
        .front()
        .is_some_and(|(deadline, _)| *deadline <= Instant::now())
    {
        entries.pop_front();
    }
    while entries.len() >= capacity {
        entries.pop_front();
    }
    entries.push_back((expires_at, value));
    true
}

fn expire<T>(entries: &mut VecDeque<(Instant, T)>, now: Instant) {
    while entries
        .front()
        .is_some_and(|(deadline, _)| *deadline <= now)
    {
        entries.pop_front();
    }
}

#[derive(Clone)]
pub struct TarpitManager {
    sender: mpsc::Sender<TcpStream>,
    enabled: bool,
}

impl TarpitManager {
    pub fn spawn(max_entries: usize, hold_for: Duration) -> Self {
        let (sender, mut receiver) = mpsc::channel::<TcpStream>(max_entries.max(1));
        tokio::spawn(async move {
            let mut entries: VecDeque<(Instant, TcpStream)> = VecDeque::new();
            // Match short configured holds with a short sweep interval so expiry
            // does not retain sockets for an unrelated fixed five-second period.
            let sweep_interval = hold_for
                .min(MAX_SWEEP_INTERVAL)
                .max(Duration::from_millis(1));
            let mut ticker = interval(sweep_interval);

            loop {
                tokio::select! {
                    maybe_stream = receiver.recv() => {
                        let Some(stream) = maybe_stream else {
                            break;
                        };
                        if max_entries == 0 || hold_for.is_zero() {
                            continue;
                        }
                        if insert_bounded(&mut entries, max_entries, Instant::now() + hold_for, stream) {
                            debug!(size = entries.len(), "connection placed in bounded silent tarpit");
                        }
                    }
                    _ = ticker.tick() => {
                        let now = Instant::now();
                        expire(&mut entries, now);
                    }
                }
            }
        });

        Self {
            sender,
            enabled: max_entries > 0 && !hold_for.is_zero(),
        }
    }

    pub async fn quarantine(&self, stream: TcpStream) {
        if !self.enabled {
            return;
        }
        let _ = self.sender.try_send(stream);
    }
}

#[cfg(test)]
mod tests {
    use super::{expire, insert_bounded};
    use std::collections::VecDeque;
    use std::time::Duration;
    use tokio::time::Instant;

    #[test]
    fn capacity_zero_never_retains_a_connection() {
        let mut entries = VecDeque::new();
        assert!(!insert_bounded(
            &mut entries,
            0,
            Instant::now() + Duration::from_secs(1),
            7
        ));
        assert!(entries.is_empty());
    }

    #[test]
    fn capacity_evicts_oldest_and_never_exceeds_limit() {
        let mut entries = VecDeque::new();
        let expiry = Instant::now() + Duration::from_secs(30);
        assert!(insert_bounded(&mut entries, 2, expiry, 1));
        assert!(insert_bounded(&mut entries, 2, expiry, 2));
        assert!(insert_bounded(&mut entries, 2, expiry, 3));
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            [2, 3]
        );
    }

    #[test]
    fn expiry_removes_dead_entries_and_keeps_live_ones() {
        let now = Instant::now();
        let mut entries = VecDeque::from([
            (now + Duration::from_secs(1), 1),
            (now + Duration::from_secs(2), 2),
        ]);
        expire(&mut entries, now + Duration::from_secs(1));
        assert_eq!(
            entries.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            [2]
        );
    }
}
