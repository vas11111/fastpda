//! Differential tests of the byte-array API against solana-pubkey's own
//! derivation: the same address, bump and error on every input.

mod common;

use common::{as_slices, check_seed_list, panic_message, solana_create, solana_try_find, Rng};
use fastpda::{bytes, Error, MAX_SEEDS, MAX_SEED_LEN};

#[test]
fn constants_match_solana() {
    assert_eq!(MAX_SEEDS, solana_pubkey::MAX_SEEDS);
    assert_eq!(MAX_SEED_LEN, solana_pubkey::MAX_SEED_LEN);
}

#[test]
fn random_seed_lists_match_solana() {
    // 0..=15 seeds of 0..=32 bytes, random program ids.
    let mut rng = Rng::new(0x5ea5_0004);
    let mut creates = 0;
    for _ in 0..20_000 {
        let count = rng.below(MAX_SEEDS);
        let seeds = rng.seeds(count, MAX_SEED_LEN);
        let program_id = rng.bytes32();
        creates += check_seed_list(&as_slices(&seeds), &program_id, &mut rng);
    }
    // Each search ends at its first off-curve bump, about 2 tries on average,
    // so each case compares about 3 creates (the found bump, the ones above
    // it, and a random one).
    assert!(creates > 50_000, "{creates} creates compared");
}

#[test]
fn common_shapes_match_solana() {
    // Shapes real programs use: one string, a string and a key, three keys
    // (an associated token account), and many keys.
    let mut rng = Rng::new(0x5ea5_0005);
    for _ in 0..2_000 {
        let program_id = rng.bytes32();
        let (a, b, c) = (rng.bytes32(), rng.bytes32(), rng.bytes32());
        let keys: Vec<[u8; 32]> = (0..15).map(|_| rng.bytes32()).collect();
        let key_slices: Vec<&[u8]> = keys.iter().map(|k| k.as_slice()).collect();
        let shapes: [&[&[u8]]; 6] = [
            &[b"config"],
            &[b"bonding-curve", &a],
            &[&a, &b, &c],
            &[b"metadata", &program_id, &a],
            &key_slices,
            &[],
        ];
        for seeds in shapes {
            check_seed_list(seeds, &program_id, &mut rng);
        }
    }
}

#[test]
fn empty_seeds_match_solana() {
    let mut rng = Rng::new(0x5ea5_0006);
    let empty: &[u8] = &[];
    for count in 0..MAX_SEEDS {
        let seeds = vec![empty; count];
        for _ in 0..20 {
            let program_id = rng.bytes32();
            check_seed_list(&seeds, &program_id, &mut rng);
        }
    }
    // Empty seeds hash to nothing: they change neither the address nor the
    // bump, only the seed count the limit sees.
    let program_id = rng.bytes32();
    let none = bytes::try_find_program_address(&[], &program_id);
    assert_eq!(
        bytes::try_find_program_address(&[empty; 15], &program_id),
        none
    );
    assert_eq!(
        bytes::try_find_program_address(&[empty; 16], &program_id),
        None
    );
}

#[test]
fn seed_boundaries_do_not_matter_like_in_solana() {
    // A PDA hashes the seeds back to back with no lengths, so ["ab", "cd"]
    // and ["abcd"] are the same address. Both sides must agree on that.
    let mut rng = Rng::new(0x5ea5_0007);
    for _ in 0..300 {
        let program_id = rng.bytes32();
        let mut joined = [0u8; 32];
        rng.fill(&mut joined);
        let cut = rng.below(33);
        let split: [&[u8]; 2] = [&joined[..cut], &joined[cut..]];
        let whole = bytes::try_find_program_address(&[&joined], &program_id);
        assert_eq!(bytes::try_find_program_address(&split, &program_id), whole);
        assert_eq!(solana_try_find(&split, &program_id), whole);
    }
}

#[test]
fn the_limits_match_solana() {
    let mut rng = Rng::new(0x5ea5_0008);
    let program_id = rng.bytes32();
    let full = [0xabu8; MAX_SEED_LEN];
    let too_long = [0xabu8; MAX_SEED_LEN + 1];
    let short = [1u8];

    // create: up to 16 seeds of up to 32 bytes.
    for count in [0, 1, 15, MAX_SEEDS, MAX_SEEDS + 1, MAX_SEEDS + 10] {
        for seed in [&full[..], &short[..], &too_long[..]] {
            let seeds = vec![seed; count];
            let ours = bytes::create_program_address(&seeds, &program_id);
            assert_eq!(
                ours,
                solana_create(&seeds, &program_id),
                "{count} seeds of {} bytes",
                seed.len()
            );
            let over = count > MAX_SEEDS || (count > 0 && seed.len() > MAX_SEED_LEN);
            assert_eq!(ours == Err(Error::MaxSeedLengthExceeded), over);
        }
    }
    // One long seed anywhere in the list is enough.
    for position in 0..MAX_SEEDS {
        let mut seeds: Vec<&[u8]> = vec![&full; MAX_SEEDS];
        seeds[position] = &too_long;
        assert_eq!(
            bytes::create_program_address(&seeds, &program_id),
            Err(Error::MaxSeedLengthExceeded)
        );
        assert_eq!(
            solana_create(&seeds, &program_id),
            Err(Error::MaxSeedLengthExceeded)
        );
    }

    // find: at most 15 seeds, since the bump takes the 16th slot.
    for count in [0, 1, 14, MAX_SEEDS - 1, MAX_SEEDS, MAX_SEEDS + 1] {
        for seed in [&full[..], &short[..], &too_long[..]] {
            let seeds = vec![seed; count];
            let ours = bytes::try_find_program_address(&seeds, &program_id);
            assert_eq!(
                ours,
                solana_try_find(&seeds, &program_id),
                "{count} seeds of {} bytes",
                seed.len()
            );
            let over = count >= MAX_SEEDS || (count > 0 && seed.len() > MAX_SEED_LEN);
            assert_eq!(ours.is_none(), over);
        }
    }
}

#[test]
fn find_panics_where_solana_panics_with_the_same_message() {
    let program_id = [9u8; 32];
    let short = [1u8];
    let too_long = [0u8; MAX_SEED_LEN + 1];
    let sixteen: Vec<&[u8]> = vec![&short; MAX_SEEDS];
    let long: Vec<&[u8]> = vec![&too_long];
    for seeds in [sixteen, long] {
        let ours = std::panic::catch_unwind(|| bytes::find_program_address(&seeds, &program_id))
            .unwrap_err();
        let theirs = std::panic::catch_unwind(|| {
            solana_pubkey::Pubkey::find_program_address(
                &seeds,
                &solana_pubkey::Pubkey::new_from_array(program_id),
            )
        })
        .unwrap_err();
        assert_eq!(panic_message(ours), panic_message(theirs));
    }
}

/// Seed lists whose first off-curve bump is far below 255, so the search
/// runs through every rejected bump above it. Found by `search_low_bumps`
/// below (the lowest of 2^27 searches was 229): seeds ["low-bump", n as 8
/// little-endian bytes] for the program id sha256("fastpda low-bump
/// search"). The bumps are also derived
/// independently by reference/pda_reference.py (see tests/known_answers.rs).
pub const LOW_BUMPS: &[(u64, u8)] = &[
    (15725687, 229),
    (41453911, 229),
    (32788965, 230),
    (9030244, 231),
    (7301109, 232),
    (7497205, 233),
    (7319664, 234),
    (1872682, 235),
];

pub fn low_bump_program() -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(b"fastpda low-bump search").into()
}

#[test]
fn low_bumps_match_solana() {
    let program_id = low_bump_program();
    for &(n, bump) in LOW_BUMPS {
        let n = n.to_le_bytes();
        let seeds: [&[u8]; 2] = [b"low-bump", &n];
        let found = bytes::try_find_program_address(&seeds, &program_id).expect("a bump");
        assert_eq!(Some(found), solana_try_find(&seeds, &program_id));
        assert_eq!(found.1, bump);
        check_seed_list(&seeds, &program_id, &mut Rng::new(n[0] as u64));
    }
}

/// Prints the seed lists for `LOW_BUMPS`:
/// `cargo test --release --test solana search_low_bumps -- --ignored --nocapture`.
#[test]
#[ignore]
fn search_low_bumps() {
    use std::sync::atomic::{AtomicU64, Ordering};
    let program_id = low_bump_program();
    const LIMIT: u64 = 1 << 27;
    const CHUNK: u64 = 1 << 16;
    let next = AtomicU64::new(0);
    let found = std::sync::Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let start = next.fetch_add(CHUNK, Ordering::Relaxed);
                if start >= LIMIT {
                    break;
                }
                for n in start..start + CHUNK {
                    let le = n.to_le_bytes();
                    let (_, bump) = bytes::find_program_address(&[b"low-bump", &le], &program_id);
                    if bump <= 236 {
                        found.lock().unwrap().push((n, bump));
                    }
                }
            });
        }
    });
    let mut found = found.into_inner().unwrap();
    found.sort_by_key(|&(n, bump)| (bump, n));
    for (n, bump) in found {
        println!("({n}, {bump}),");
    }
}
