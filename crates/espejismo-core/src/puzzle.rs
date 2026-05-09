use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PuzzleConfig {
    pub difficulty_bits: u8,
}

impl PuzzleConfig {
    pub fn capped(self) -> Self {
        Self {
            difficulty_bits: self.difficulty_bits.min(24),
        }
    }
}

pub fn solve(
    mut body: Vec<u8>,
    nonce_range: std::ops::Range<usize>,
    difficulty_bits: u8,
) -> Vec<u8> {
    let difficulty_bits = difficulty_bits.min(24);
    if difficulty_bits == 0 {
        return body;
    }

    let mut nonce = 0_u64;
    loop {
        body[nonce_range.clone()].copy_from_slice(&nonce.to_be_bytes());
        if verify(&body, difficulty_bits) {
            return body;
        }
        nonce = nonce.wrapping_add(1);
    }
}

pub fn verify(body: &[u8], difficulty_bits: u8) -> bool {
    let difficulty_bits = difficulty_bits.min(24);
    if difficulty_bits == 0 {
        return true;
    }

    let digest = Sha256::digest(body);
    leading_zero_bits(&digest) >= difficulty_bits
}

fn leading_zero_bits(bytes: &[u8]) -> u8 {
    let mut total = 0_u8;
    for byte in bytes {
        if *byte == 0 {
            total += 8;
        } else {
            total += byte.leading_zeros() as u8;
            break;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::{solve, verify};

    #[test]
    fn solves_and_verifies_small_puzzle() {
        let mut body = vec![0_u8; 32];
        body[0] = 42;
        let solved = solve(body, 8..16, 8);
        assert!(verify(&solved, 8));
    }

    #[test]
    fn zero_difficulty_always_passes() {
        assert!(verify(b"anything", 0));
    }
}
