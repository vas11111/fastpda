//! Solana program-derived addresses (PDAs), bit for bit the same as
//! `solana-pubkey`'s, about three times faster.
//!
//! A PDA is `sha256(seeds || program_id || "ProgramDerivedAddress")`, kept only
//! when those 32 bytes are **not** the encoding of an ed25519 point, so that no
//! private key can exist for it. `find_program_address` appends a one-byte
//! "bump" seed and tries 255, 254, ... down to 1 until the hash is off the
//! curve. Each try is one SHA-256 (tens of nanoseconds with hardware SHA) and
//! one on-curve test, and the on-curve test is where the time goes:
//! `solana-pubkey` asks curve25519-dalek to decompress the point, which is a
//! field exponentiation of about 254 squarings, roughly 97% of a try.
//!
//! This crate answers the same yes/no question from a Jacobi symbol:
//!
//! - y encodes a point exactly when `(y^2 - 1) * (d*y^2 + 1)` is 0 or a square
//!   mod p = 2^255 - 19 (see [`is_on_curve`] for why);
//! - that product is three field operations, done with fiat-crypto's formally
//!   verified arithmetic;
//! - whether it is a square is decided by the binary Jacobi-symbol algorithm:
//!   shifts, comparisons and subtractions, no exponentiation.
//!
//! `find_program_address` also absorbs the seeds into SHA-256 once and
//! clones that state for each bump, instead of re-hashing the seeds per try.
//!
//! # API
//!
//! With the default `solana-pubkey` feature, the crate root has drop-in
//! replacements for `Pubkey::find_program_address`,
//! `Pubkey::try_find_program_address` and `Pubkey::create_program_address`:
//! same arguments, same results, same errors, same panics.
//!
//! [`bytes`] has the same three functions over `[u8; 32]`, with no Solana
//! dependency (build with `default-features = false`). [`is_on_curve`] is the
//! on-curve test on its own.
//!
//! ```
//! # #[cfg(feature = "solana-pubkey")] {
//! use solana_pubkey::Pubkey;
//!
//! let program_id = Pubkey::new_from_array([7; 32]);
//! let user = Pubkey::new_from_array([42; 32]);
//! let (vault, bump) = fastpda::find_program_address(&[b"vault", user.as_ref()], &program_id);
//!
//! // The same answer as solana-pubkey's own derivation, just faster.
//! let again = fastpda::create_program_address(&[b"vault", user.as_ref(), &[bump]], &program_id);
//! assert_eq!(again, Ok(vault));
//! assert!(!fastpda::is_on_curve(vault.as_array()));
//! # }
//! ```
//!
//! # Timing
//!
//! Nothing here is constant-time: the Jacobi loop branches on its inputs and
//! its running time depends on them. That is fine for PDAs, whose seeds and
//! program ids are public by construction. Do not use [`is_on_curve`] on
//! secret data.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod bytes;
mod curve;
mod jacobi;
#[cfg(feature = "solana-pubkey")]
mod pubkey;
#[cfg(test)]
#[path = "tests/util.rs"]
mod test_util;

use core::fmt;

pub use curve::is_on_curve;
#[cfg(feature = "solana-pubkey")]
pub use pubkey::{create_program_address, find_program_address, try_find_program_address};

/// The most seeds a PDA can have, counting the bump seed. Equal to
/// `solana_pubkey::MAX_SEEDS`.
pub const MAX_SEEDS: usize = 16;

/// The longest a single seed can be, in bytes. Equal to
/// `solana_pubkey::MAX_SEED_LEN`.
pub const MAX_SEED_LEN: usize = 32;

/// The bytes hashed after the seeds and the program id.
pub const PDA_MARKER: &[u8; 21] = b"ProgramDerivedAddress";

/// Why [`bytes::create_program_address`] gave no address. The variants mirror
/// solana-pubkey's `PubkeyError` (which also has `IllegalOwner`, an error PDA
/// derivation never returns).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Error {
    /// More than [`MAX_SEEDS`] seeds, or a seed longer than [`MAX_SEED_LEN`]
    /// bytes.
    MaxSeedLengthExceeded,
    /// The hash is an ed25519 point, so it cannot be a PDA.
    InvalidSeeds,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The same messages as solana-pubkey's `PubkeyError`.
        f.write_str(match self {
            Error::MaxSeedLengthExceeded => "Length of the seed is too long for address generation",
            Error::InvalidSeeds => "Provided seeds do not result in a valid address",
        })
    }
}

impl core::error::Error for Error {}
