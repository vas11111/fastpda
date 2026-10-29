//! The `Pubkey` API at the crate root against solana-pubkey's own methods:
//! same results, same `PubkeyError`s, same panic message.

#![cfg(feature = "solana-pubkey")]

mod common;

use common::{as_slices, panic_message, Rng};
use fastpda::{Error, MAX_SEEDS, MAX_SEED_LEN};
use solana_pubkey::{Pubkey, PubkeyError};

#[test]
fn random_seed_lists_match_solana() {
    let mut rng = Rng::new(0x5ea5_0009);
    for _ in 0..10_000 {
        // Up to 17 seeds of up to 34 bytes: some cases break the limits.
        let count = rng.below(MAX_SEEDS + 2);
        let seeds = rng.seeds(count, MAX_SEED_LEN + 2);
        let seeds = as_slices(&seeds);
        let program_id = Pubkey::new_from_array(rng.bytes32());

        let found = fastpda::try_find_program_address(&seeds, &program_id);
        assert_eq!(found, Pubkey::try_find_program_address(&seeds, &program_id));
        if let Some(found) = found {
            assert_eq!(fastpda::find_program_address(&seeds, &program_id), found);
            assert!(!found.0.is_on_curve());
        }

        let bump = [rng.next_u64() as u8];
        let mut with_bump = seeds.clone();
        with_bump.push(&bump);
        for seeds in [&seeds, &with_bump] {
            assert_eq!(
                fastpda::create_program_address(seeds, &program_id),
                Pubkey::create_program_address(seeds, &program_id),
            );
        }
    }
}

#[test]
fn errors_convert_to_solanas() {
    assert_eq!(
        PubkeyError::from(Error::MaxSeedLengthExceeded),
        PubkeyError::MaxSeedLengthExceeded
    );
    assert_eq!(
        PubkeyError::from(Error::InvalidSeeds),
        PubkeyError::InvalidSeeds
    );
    for (ours, theirs) in [
        (
            Error::MaxSeedLengthExceeded,
            PubkeyError::MaxSeedLengthExceeded,
        ),
        (Error::InvalidSeeds, PubkeyError::InvalidSeeds),
    ] {
        assert_eq!(ours.to_string(), theirs.to_string());
    }
}

#[test]
fn find_panics_like_solana() {
    let program_id = Pubkey::new_from_array([3; 32]);
    let short = [1u8];
    let sixteen: Vec<&[u8]> = vec![&short; MAX_SEEDS];
    let ours = std::panic::catch_unwind(|| fastpda::find_program_address(&sixteen, &program_id))
        .unwrap_err();
    let theirs = std::panic::catch_unwind(|| Pubkey::find_program_address(&sixteen, &program_id))
        .unwrap_err();
    assert_eq!(panic_message(ours), panic_message(theirs));
}

#[test]
fn is_on_curve_matches_pubkey_is_on_curve() {
    let mut rng = Rng::new(0x5ea5_000a);
    for _ in 0..5_000 {
        let key = Pubkey::new_from_array(rng.bytes32());
        assert_eq!(fastpda::is_on_curve(key.as_array()), key.is_on_curve());
    }
}
