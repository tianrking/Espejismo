use std::collections::{HashSet, VecDeque};

use anyhow::{bail, Result};

#[derive(Debug)]
pub struct ReplayCache {
    ttl_secs: i64,
    seen: HashSet<[u8; 32]>,
    order: VecDeque<(i64, [u8; 32])>,
}

impl ReplayCache {
    pub fn new(ttl_secs: i64) -> Self {
        Self {
            ttl_secs,
            seen: HashSet::new(),
            order: VecDeque::new(),
        }
    }

    pub fn check_and_insert(&mut self, now: i64, public_key: [u8; 32]) -> Result<()> {
        self.prune(now);
        if self.seen.contains(&public_key) {
            bail!("replayed ephemeral public key");
        }
        self.seen.insert(public_key);
        self.order.push_back((now, public_key));
        Ok(())
    }

    fn prune(&mut self, now: i64) {
        while let Some((inserted_at, key)) = self.order.front().copied() {
            if now - inserted_at <= self.ttl_secs {
                break;
            }
            self.order.pop_front();
            self.seen.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ReplayCache;

    #[test]
    fn rejects_repeated_key_inside_window() {
        let mut cache = ReplayCache::new(60);
        let key = [7_u8; 32];
        cache.check_and_insert(100, key).unwrap();
        assert!(cache.check_and_insert(120, key).is_err());
    }

    #[test]
    fn allows_key_after_window_expires() {
        let mut cache = ReplayCache::new(60);
        let key = [7_u8; 32];
        cache.check_and_insert(100, key).unwrap();
        cache.check_and_insert(161, key).unwrap();
    }
}
