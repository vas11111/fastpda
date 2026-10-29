//! PDA derivation over plain `[u8; 32]` keys, with no Solana dependency.
//!
//! These are the functions the `Pubkey` ones at the crate root wrap. They take
//! and return addresses as 32-byte arrays and report errors as
//! [`Error`]. Use them without the `solana-pubkey` feature, or
//! with a `Pubkey` type from another solana-pubkey major version
//! (`Pubkey::new_from_array` / `to_bytes` convert).

use sha2::{Digest, Sha256};

use crate::{is_on_curve, Error, MAX_SEEDS, MAX_SEED_LEN, PDA_MARKER};

/// The address for these seeds (the bump, if any, included as the last seed),
/// or an error when there is none.
///
/// Matches solana-pubkey's `Pubkey::create_program_address`:
/// - more than [`MAX_SEEDS`] seeds, or any seed longer than [`MAX_SEED_LEN`]
///   bytes: [`Error::MaxSeedLengthExceeded`];
/// - the hash is an ed25519 point: [`Error::InvalidSeeds`].
pub fn create_program_address(seeds: &[&[u8]], program_id: &[u8; 32]) -> Result<[u8; 32], Error> {
    if seeds.len() > MAX_SEEDS || seeds.iter().any(|seed| seed.len() > MAX_SEED_LEN) {
        return Err(Error::MaxSeedLengthExceeded);
    }
    let mut hasher = Sha256::new();
    for seed in seeds {
        hasher.update(seed);
    }
    hasher.update(program_id);
    hasher.update(PDA_MARKER);
    let hash: [u8; 32] = hasher.finalize().into();
    if is_on_curve(&hash) {
        return Err(Error::InvalidSeeds);
    }
    Ok(hash)
}

/// The first bump seed from 255 down to 1 that gives an address, with that
/// address; `None` when no bump does or the seeds break the limits.
///
/// Matches solana-pubkey's `Pubkey::try_find_program_address`, including its
/// limits: the bump is one more seed, so `seeds` may hold at most
/// `MAX_SEEDS - 1` (15) seeds of at most [`MAX_SEED_LEN`] bytes each. Bump 0
/// is never tried, as in solana-pubkey.
pub fn try_find_program_address(seeds: &[&[u8]], program_id: &[u8; 32]) -> Option<([u8; 32], u8)> {
    if seeds.len() >= MAX_SEEDS || seeds.iter().any(|seed| seed.len() > MAX_SEED_LEN) {
        return None;
    }
    // SHA-256 is a stream: hashing the seeds once and cloning that state for
    // each bump gives the same digests as hashing everything per try.
    let mut seeded = Sha256::new();
    for seed in seeds {
        seeded.update(seed);
    }
    for bump in (1..=u8::MAX).rev() {
        let mut hasher = seeded.clone();
        hasher.update([bump]);
        hasher.update(program_id);
        hasher.update(PDA_MARKER);
        let hash: [u8; 32] = hasher.finalize().into();
        if !is_on_curve(&hash) {
            return Some((hash, bump));
        }
    }
    // Each try is off the curve with probability about 1/2, so 255 misses in a
    // row (about 2^-255) does not happen in practice.
    None
}

/// [`try_find_program_address`], panicking where it returns `None`.
///
/// Matches solana-pubkey's `Pubkey::find_program_address`, including the
/// panic message.
///
/// # Panics
///
/// When `seeds` holds [`MAX_SEEDS`] or more seeds, when a seed is longer than
/// [`MAX_SEED_LEN`] bytes, or (with probability about 2^-255) when no bump
/// gives an address.
pub fn find_program_address(seeds: &[&[u8]], program_id: &[u8; 32]) -> ([u8; 32], u8) {
    try_find_program_address(seeds, program_id)
        .unwrap_or_else(|| panic!("Unable to find a viable program address bump seed"))
}
