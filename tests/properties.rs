//! Property tests (proptest): on random and shrunk inputs, fastpda agrees with
//! solana-pubkey and curve25519-dalek, and its results hang together.

mod common;

use common::{as_slices, dalek_on_curve, solana_create, solana_try_find};
use fastpda::{bytes, is_on_curve, Error, MAX_SEEDS, MAX_SEED_LEN};
use proptest::{collection::vec, prelude::*};

/// Seed lists within find's limits: up to 15 seeds of up to 32 bytes.
fn seed_lists() -> impl Strategy<Value = Vec<Vec<u8>>> {
    vec(vec(any::<u8>(), 0..=MAX_SEED_LEN), 0..MAX_SEEDS)
}

/// Seed lists around create's limits: up to 18 seeds of up to 34 bytes.
fn any_seed_lists() -> impl Strategy<Value = Vec<Vec<u8>>> {
    vec(vec(any::<u8>(), 0..=MAX_SEED_LEN + 2), 0..=MAX_SEEDS + 2)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4_096))]

    #[test]
    fn on_curve_matches_dalek(bytes in any::<[u8; 32]>()) {
        prop_assert_eq!(is_on_curve(&bytes), dalek_on_curve(&bytes));
    }

    #[test]
    fn the_sign_bit_never_matters(bytes in any::<[u8; 32]>()) {
        let mut flipped = bytes;
        flipped[31] ^= 0x80;
        prop_assert_eq!(is_on_curve(&bytes), is_on_curve(&flipped));
    }

    #[test]
    fn non_canonical_y_reads_as_y_minus_p(k in 0u8..19, sign in any::<bool>()) {
        // p + k against k, for every k that keeps p + k below 2^255.
        let mut non_canonical = [0xffu8; 32];
        non_canonical[0] = 0xed + k;
        non_canonical[31] = 0x7f | if sign { 0x80 } else { 0 };
        let mut canonical = [0u8; 32];
        canonical[0] = k;
        canonical[31] = if sign { 0x80 } else { 0 };
        prop_assert_eq!(is_on_curve(&non_canonical), is_on_curve(&canonical));
        prop_assert_eq!(is_on_curve(&non_canonical), dalek_on_curve(&non_canonical));
    }

    #[test]
    fn find_matches_solana(seeds in seed_lists(), program_id in any::<[u8; 32]>()) {
        let seeds = as_slices(&seeds);
        prop_assert_eq!(bytes::try_find_program_address(&seeds, &program_id), solana_try_find(&seeds, &program_id));
    }

    #[test]
    fn create_matches_solana(seeds in any_seed_lists(), program_id in any::<[u8; 32]>()) {
        let seeds = as_slices(&seeds);
        prop_assert_eq!(bytes::create_program_address(&seeds, &program_id), solana_create(&seeds, &program_id));
    }

    #[test]
    fn find_returns_the_first_bump_create_accepts(seeds in seed_lists(), program_id in any::<[u8; 32]>()) {
        let seeds = as_slices(&seeds);
        let (address, bump) = bytes::find_program_address(&seeds, &program_id);
        prop_assert!(!is_on_curve(&address));
        prop_assert!(bump >= 1);
        for b in bump..=u8::MAX {
            let bump_seed = [b];
            let mut with_bump = seeds.clone();
            with_bump.push(&bump_seed);
            let created = bytes::create_program_address(&with_bump, &program_id);
            if b == bump {
                prop_assert_eq!(created, Ok(address));
            } else {
                prop_assert_eq!(created, Err(Error::InvalidSeeds));
            }
        }
    }

    #[test]
    fn create_fails_exactly_on_the_curve(seeds in seed_lists(), bump in any::<u8>(), program_id in any::<[u8; 32]>()) {
        // Within the limits, create fails exactly when the hash is a point.
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for seed in &seeds {
            hasher.update(seed);
        }
        hasher.update([bump]);
        hasher.update(program_id);
        hasher.update(fastpda::PDA_MARKER);
        let hash: [u8; 32] = hasher.finalize().into();
        let mut seeds = as_slices(&seeds);
        let bump_seed = [bump];
        seeds.push(&bump_seed);
        let expected = if dalek_on_curve(&hash) { Err(Error::InvalidSeeds) } else { Ok(hash) };
        prop_assert_eq!(bytes::create_program_address(&seeds, &program_id), expected);
    }

    #[test]
    fn seed_boundaries_do_not_matter(joined in vec(any::<u8>(), 0..=MAX_SEED_LEN), cut in 0usize..=MAX_SEED_LEN, program_id in any::<[u8; 32]>()) {
        let cut = cut.min(joined.len());
        let split: [&[u8]; 2] = [&joined[..cut], &joined[cut..]];
        prop_assert_eq!(
            bytes::try_find_program_address(&split, &program_id),
            bytes::try_find_program_address(&[&joined], &program_id),
        );
    }
}
