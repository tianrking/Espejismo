use std::collections::VecDeque;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{interval, Instant};
use tracing::debug;

#[derive(Clone)]
pub struct TarpitManager {
    sender: mpsc::Sender<TcpStream>,
}

impl TarpitManager {
    pub fn spawn(max_entries: usize, hold_for: Duration) -> Self {
        let (sender, mut receiver) = mpsc::channel::<TcpStream>(max_entries.max(1));
        tokio::spawn(async move {
            let mut entries: VecDeque<(Instant, TcpStream)> = VecDeque::new();
            let mut ticker = interval(Duration::from_secs(5));

            loop {
                tokio::select! {
                    maybe_stream = receiver.recv() => {
                        let Some(stream) = maybe_stream else {
                            break;
                        };
                        if max_entries == 0 || hold_for.is_zero() {
                            continue;
                        }
                        while entries.len() >= max_entries {
                            entries.pop_front();
                        }
                        entries.push_back((Instant::now() + hold_for, stream));
                        debug!(size = entries.len(), "connection placed in bounded silent tarpit");
                    }
                    _ = ticker.tick() => {
                        let now = Instant::now();
                        while entries.front().is_some_and(|(expires_at, _)| *expires_at <= now) {
                            entries.pop_front();
                        }
                    }
                }
            }
        });

        Self { sender }
    }

    pub async fn quarantine(&self, stream: TcpStream) {
        let _ = self.sender.try_send(stream);
    }
}
