//! Shared by the integration tests: a seeded generator, random inputs, and
//! the reference implementations (solana-pubkey and curve25519-dalek) in the
//! byte-array shapes fastpda's `bytes` API uses.

#![allow(dead_code)]

use curve25519_dalek::edwards::CompressedEdwardsY;
use fastpda::Error;
use solana_pubkey::{Pubkey, PubkeyError};

/// SplitMix64: a small, fixed generator, so a seed names the same inputs on
/// every platform and with every version of every dependency.
#[derive(Clone)]
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

    /// Uniform in 0..n (n > 0); the tiny modulo bias does not matter here.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    pub fn bytes32(&mut self) -> [u8; 32] {
        let mut out = [0u8; 32];
        self.fill(&mut out);
        out
    }

    pub fn fill(&mut self, out: &mut [u8]) {
        for chunk in out.chunks_mut(8) {
            let word = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
    }

    /// `count` seeds; each is 32 bytes half of the time (the common shape, a
    /// key), otherwise 0..=`max_len` bytes.
    pub fn seeds(&mut self, count: usize, max_len: usize) -> Vec<Vec<u8>> {
        (0..count)
            .map(|_| {
                let len = if self.next_u64() & 1 == 0 {
                    32.min(max_len)
                } else {
                    self.below(max_len + 1)
                };
                let mut seed = vec![0u8; len];
                self.fill(&mut seed);
                seed
            })
            .collect()
    }
}

/// A fresh random u64 from the standard library's per-process hash keys.
pub fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(std::process::id() as u64);
    hasher.finish()
}

/// The seed for a randomized run: `FASTPDA_SEED` (decimal or 0x-hex) if set,
/// else a fresh random one. Printed either way, so a failure can be rerun.
pub fn run_seed(name: &str) -> u64 {
    let seed = match std::env::var("FASTPDA_SEED") {
        Ok(s) => {
            let s = s.trim();
            match s.strip_prefix("0x") {
                Some(hex) => u64::from_str_radix(hex, 16).expect("FASTPDA_SEED"),
                None => s.parse().expect("FASTPDA_SEED"),
            }
        }
        Err(_) => random_u64(),
    };
    println!("{name}: seed {seed:#018x} (rerun with FASTPDA_SEED={seed:#018x})");
    seed
}

pub fn as_slices(seeds: &[Vec<u8>]) -> Vec<&[u8]> {
    seeds.iter().map(|s| s.as_slice()).collect()
}

/// The reference on-curve test: what solana-pubkey's `bytes_are_curve_point`
/// runs.
pub fn dalek_on_curve(bytes: &[u8; 32]) -> bool {
    CompressedEdwardsY(*bytes).decompress().is_some()
}

pub fn to_error(error: PubkeyError) -> Error {
    match error {
        PubkeyError::MaxSeedLengthExceeded => Error::MaxSeedLengthExceeded,
        PubkeyError::InvalidSeeds => Error::InvalidSeeds,
        other => panic!("PDA derivation returned {other:?}"),
    }
}

/// solana-pubkey's `create_program_address`, in the shape of fastpda's
/// `bytes::create_program_address`.
pub fn solana_create(seeds: &[&[u8]], program_id: &[u8; 32]) -> Result<[u8; 32], Error> {
    Pubkey::create_program_address(seeds, &Pubkey::new_from_array(*program_id))
        .map(|key| key.to_bytes())
        .map_err(to_error)
}

/// solana-pubkey's `try_find_program_address`, in the shape of fastpda's.
pub fn solana_try_find(seeds: &[&[u8]], program_id: &[u8; 32]) -> Option<([u8; 32], u8)> {
    Pubkey::try_find_program_address(seeds, &Pubkey::new_from_array(*program_id))
        .map(|(key, bump)| (key.to_bytes(), bump))
}

/// One differential case: `try_find` against solana's; then `create` against
/// solana's with the found bump, with every bump above it (each of which
/// solana rejected while searching), and with one random bump. Returns the
/// number of on-curve tests the case compared.
pub fn check_seed_list(seeds: &[&[u8]], program_id: &[u8; 32], rng: &mut Rng) -> u64 {
    let found = fastpda::bytes::try_find_program_address(seeds, program_id);
    assert_eq!(
        found,
        solana_try_find(seeds, program_id),
        "seeds {seeds:02x?} program {program_id:02x?}"
    );
    let Some((address, bump)) = found else {
        return 0;
    };
    assert!(!dalek_on_curve(&address));

    let mut compared = 0;
    let random_bump = rng.next_u64() as u8;
    for b in (bump..=u8::MAX).chain([random_bump]) {
        let bump_seed = [b];
        let mut with_bump = seeds.to_vec();
        with_bump.push(&bump_seed);
        let ours = fastpda::bytes::create_program_address(&with_bump, program_id);
        assert_eq!(
            ours,
            solana_create(&with_bump, program_id),
            "seeds {with_bump:02x?} program {program_id:02x?}"
        );
        if b == bump {
            assert_eq!(ours, Ok(address));
        } else if b > bump {
            assert_eq!(ours, Err(Error::InvalidSeeds));
        }
        compared += 1;
    }
    compared
}

/// The message of a caught panic.
pub fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(s) => *s,
        Err(payload) => payload
            .downcast_ref::<&str>()
            .expect("a string panic")
            .to_string(),
    }
}

/// Parses lowercase or uppercase hex.
pub fn hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "odd-length hex {s:?}");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

pub fn hex32(s: &str) -> [u8; 32] {
    hex(s).try_into().expect("32 bytes of hex")
}
