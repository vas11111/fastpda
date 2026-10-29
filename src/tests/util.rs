//! Helpers for the unit tests: a seeded generator and big-integer references.

use num_bigint::BigUint;

/// SplitMix64: a small, fixed generator, so a seed names the same inputs on
/// every platform and every version of every dependency.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn bytes32(&mut self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for chunk in out.chunks_mut(8) {
            chunk.copy_from_slice(&self.next_u64().to_le_bytes());
        }
        out
    }
}

/// p = 2^255 - 19.
pub fn p() -> BigUint {
    (BigUint::from(1u8) << 255u32) - 19u32
}

pub fn from_le(bytes: &[u8; 32]) -> BigUint {
    BigUint::from_bytes_le(bytes)
}

/// `x` as 32 little-endian bytes; `x` must be below 2^256.
pub fn to_le(x: &BigUint) -> [u8; 32] {
    let v = x.to_bytes_le();
    assert!(v.len() <= 32);
    let mut out = [0u8; 32];
    out[..v.len()].copy_from_slice(&v);
    out
}

/// Euler's criterion, the textbook definition: a^((p-1)/2) mod p is 0 for a
/// multiple of p, 1 for a nonzero square and p-1 otherwise. Returns whether
/// `a` is 0 or a square mod p.
pub fn euler_is_square(a: &BigUint) -> bool {
    let p = p();
    let r = a.modpow(&((&p - 1u32) >> 1u32), &p);
    assert!(r == BigUint::from(0u8) || r == BigUint::from(1u8) || r == &p - 1u32);
    r != &p - 1u32
}

/// The seed for a randomized run: `FASTPDA_SEED` (decimal or 0x-hex) if set,
/// else a fresh random one. Printed either way, so a failure can be rerun.
pub fn run_seed(name: &str) -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let seed = match std::env::var("FASTPDA_SEED") {
        Ok(s) => match s.trim().strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16).expect("FASTPDA_SEED"),
            None => s.trim().parse().expect("FASTPDA_SEED"),
        },
        Err(_) => std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish(),
    };
    println!("{name}: seed {seed:#018x} (rerun with FASTPDA_SEED={seed:#018x})");
    seed
}
