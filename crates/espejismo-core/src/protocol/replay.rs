use std::collections::{HashSet, VecDeque};

use anyhow::{bail, Result};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ReplayKey {
    EphemeralPublicKey([u8; 32]),
    FirstPacketDigest([u8; 32]),
}

#[derive(Debug)]
pub struct ReplayCache {
    ttl_secs: i64,
    seen: HashSet<ReplayKey>,
    order: VecDeque<(i64, ReplayKey)>,
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
        self.check_and_insert_ephemeral_public_key(now, public_key)
    }

    pub fn check_and_insert_ephemeral_public_key(
        &mut self,
        now: i64,
        public_key: [u8; 32],
    ) -> Result<()> {
        self.check_and_insert_key(now, ReplayKey::EphemeralPublicKey(public_key))
            .map_err(|_| anyhow::anyhow!("replayed ephemeral public key"))
    }

    pub fn check_and_insert_first_packet_digest(
        &mut self,
        now: i64,
        digest: [u8; 32],
    ) -> Result<()> {
        self.check_and_insert_key(now, ReplayKey::FirstPacketDigest(digest))
            .map_err(|_| anyhow::anyhow!("replayed first packet digest"))
    }

    /// Records both authenticated identifiers together so a failed replay
    /// check cannot leave only one half of a handshake in the cache.
    pub fn check_and_insert_handshake(
        &mut self,
        now: i64,
        digest: [u8; 32],
        public_key: [u8; 32],
    ) -> Result<()> {
        self.prune(now);
        let digest_key = ReplayKey::FirstPacketDigest(digest);
        let public_key_key = ReplayKey::EphemeralPublicKey(public_key);
        if self.seen.contains(&digest_key) {
            bail!("replayed first packet digest");
        }
        if self.seen.contains(&public_key_key) {
            bail!("replayed ephemeral public key");
        }
        for key in [digest_key, public_key_key] {
            self.seen.insert(key);
            self.order.push_back((now, key));
        }
        Ok(())
    }

    fn check_and_insert_key(&mut self, now: i64, key: ReplayKey) -> Result<()> {
        self.prune(now);
        if self.seen.contains(&key) {
            bail!("replayed key");
        }
        self.seen.insert(key);
        self.order.push_back((now, key));
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

    #[test]
    fn rejects_repeated_first_packet_digest_inside_window() {
        let mut cache = ReplayCache::new(60);
        let digest = [9_u8; 32];
        cache
            .check_and_insert_first_packet_digest(100, digest)
            .unwrap();
        let err = cache
            .check_and_insert_first_packet_digest(120, digest)
            .unwrap_err()
            .to_string();
        assert!(err.contains("first packet"), "{err}");
    }

    #[test]
    fn first_packet_digest_and_public_key_do_not_collide() {
        let mut cache = ReplayCache::new(60);
        let same_bytes = [11_u8; 32];
        cache
            .check_and_insert_first_packet_digest(100, same_bytes)
            .unwrap();
        cache.check_and_insert(100, same_bytes).unwrap();
    }

    #[test]
    fn rejected_handshake_does_not_partially_insert_its_digest() {
        let mut cache = ReplayCache::new(60);
        let seen_public_key = [12_u8; 32];
        let digest = [13_u8; 32];
        cache.check_and_insert(100, seen_public_key).unwrap();

        assert!(cache
            .check_and_insert_handshake(101, digest, seen_public_key)
            .is_err());
        cache
            .check_and_insert_handshake(102, digest, [14_u8; 32])
            .unwrap();
    }
}
