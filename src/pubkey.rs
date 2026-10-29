//! The `Pubkey` API: drop-in replacements for solana-pubkey's PDA functions.

use solana_pubkey::{Pubkey, PubkeyError};

use crate::{bytes, Error};

impl From<Error> for PubkeyError {
    fn from(error: Error) -> Self {
        match error {
            Error::MaxSeedLengthExceeded => PubkeyError::MaxSeedLengthExceeded,
            Error::InvalidSeeds => PubkeyError::InvalidSeeds,
        }
    }
}

/// `Pubkey::create_program_address`, faster: the address for these seeds (the
/// bump, if any, included as the last seed).
///
/// # Errors
///
/// The same as solana-pubkey's:
/// - `PubkeyError::MaxSeedLengthExceeded` for more than
///   [`MAX_SEEDS`](crate::MAX_SEEDS) seeds or a seed longer than
///   [`MAX_SEED_LEN`](crate::MAX_SEED_LEN) bytes;
/// - `PubkeyError::InvalidSeeds` when the hash is an ed25519 point.
pub fn create_program_address(seeds: &[&[u8]], program_id: &Pubkey) -> Result<Pubkey, PubkeyError> {
    bytes::create_program_address(seeds, program_id.as_array())
        .map(Pubkey::new_from_array)
        .map_err(PubkeyError::from)
}

/// `Pubkey::try_find_program_address`, faster: the address and bump seed for
/// the first bump from 255 down to 1 that gives one.
///
/// Returns `None` where solana-pubkey's does: when `seeds` holds
/// [`MAX_SEEDS`](crate::MAX_SEEDS) or more seeds (the bump needs a slot), when
/// a seed is longer than [`MAX_SEED_LEN`](crate::MAX_SEED_LEN) bytes, or
/// (with probability about 2^-255) when no bump works.
pub fn try_find_program_address(seeds: &[&[u8]], program_id: &Pubkey) -> Option<(Pubkey, u8)> {
    bytes::try_find_program_address(seeds, program_id.as_array())
        .map(|(address, bump)| (Pubkey::new_from_array(address), bump))
}

/// `Pubkey::find_program_address`, faster: the address and bump seed for the
/// first bump from 255 down to 1 that gives one.
///
/// # Panics
///
/// Where solana-pubkey's does, with the same message: when
/// [`try_find_program_address`] would return `None`.
pub fn find_program_address(seeds: &[&[u8]], program_id: &Pubkey) -> (Pubkey, u8) {
    let (address, bump) = bytes::find_program_address(seeds, program_id.as_array());
    (Pubkey::new_from_array(address), bump)
}
