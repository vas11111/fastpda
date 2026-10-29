//! Long randomized sweeps against the references, ignored by default:
//!
//! ```text
//! cargo test --release -- --ignored sweep --nocapture --test-threads=1
//! ```
//!
//! Each sweep uses every core (hence one test at a time) and prints its seed;
//! `FASTPDA_SEED=<seed>` reruns the same inputs.
//! The work is split into chunks, each with a generator seeded from the run
//! seed and the chunk number, so a seed names the same inputs on any machine
//! and with any number of threads.

mod common;

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use common::{as_slices, check_seed_list, dalek_on_curve, run_seed, solana_create, Rng};
use fastpda::{bytes, is_on_curve, MAX_SEEDS, MAX_SEED_LEN};

const CHUNK: u64 = 100_000;

/// Runs `cases` cases on every core, `case` taking each chunk's generator;
/// returns the sum of what `case` returns.
fn sweep(name: &str, cases: u64, case: impl Fn(&mut Rng) -> u64 + Sync) -> u64 {
    let seed = run_seed(name);
    let start = Instant::now();
    let next_chunk = AtomicU64::new(0);
    let total = AtomicU64::new(0);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let chunk = next_chunk.fetch_add(1, Ordering::Relaxed);
                if chunk * CHUNK >= cases {
                    break;
                }
                // A different stream per chunk: mix the chunk number in.
                let mut rng = Rng::new(seed ^ chunk.wrapping_mul(0xd1b5_4a32_d192_ed03));
                rng.next_u64();
                let mut sum = 0;
                for _ in 0..CHUNK.min(cases - chunk * CHUNK) {
                    sum += case(&mut rng);
                }
                total.fetch_add(sum, Ordering::Relaxed);
            });
        }
    });
    println!(
        "{name}: {cases} cases on {threads} threads in {:.1} s",
        start.elapsed().as_secs_f64()
    );
    total.into_inner()
}

/// A 32-byte string: mostly uniform, with some near p, past p, and sparse.
fn curve_input(rng: &mut Rng) -> [u8; 32] {
    let mut bytes = rng.bytes32();
    match rng.next_u64() % 16 {
        // y = p - k for k < 4096, or p + k for k < 19 (the non-canonical
        // encodings), with a random sign bit. p's low two bytes are ed ff,
        // the rest ff, the top 7f.
        0 => {
            let low: u16 = if rng.next_u64() & 1 == 0 {
                0xffed - rng.below(4096) as u16
            } else {
                0xffed + rng.below(19) as u16
            };
            let mut y = [0xffu8; 32];
            y[..2].copy_from_slice(&low.to_le_bytes());
            y[31] = 0x7f | (bytes[31] & 0x80);
            bytes = y;
        }
        // A few bits set.
        1 => {
            let mut sparse = [0u8; 32];
            for _ in 0..rng.below(4) + 1 {
                let bit = rng.below(256);
                sparse[bit / 8] |= 1 << (bit % 8);
            }
            bytes = sparse;
        }
        _ => {}
    }
    bytes
}

#[test]
#[ignore]
fn sweep_on_curve_matches_dalek() {
    const CASES: u64 = 100_000_000;
    let on = sweep("sweep_on_curve_matches_dalek", CASES, |rng| {
        let bytes = curve_input(rng);
        let expected = dalek_on_curve(&bytes);
        assert_eq!(is_on_curve(&bytes), expected, "bytes {bytes:02x?}");
        expected as u64
    });
    println!("  {on} of {CASES} on the curve");
    assert!(on > CASES * 2 / 5 && on < CASES * 3 / 5);
}

#[test]
#[ignore]
fn sweep_seed_lists_match_solana() {
    const CASES: u64 = 10_000_000;
    let creates = sweep("sweep_seed_lists_match_solana", CASES, |rng| {
        let count = rng.below(MAX_SEEDS);
        let seeds = rng.seeds(count, MAX_SEED_LEN);
        let program_id = rng.bytes32();
        let mut compared = check_seed_list(&as_slices(&seeds), &program_id, rng);
        // One in eight also tries a list that may break the limits.
        if rng.next_u64() % 8 == 0 {
            let count = rng.below(MAX_SEEDS + 3);
            let seeds = rng.seeds(count, MAX_SEED_LEN + 1);
            let seeds = as_slices(&seeds);
            assert_eq!(
                bytes::create_program_address(&seeds, &program_id),
                solana_create(&seeds, &program_id)
            );
            compared += 1;
        }
        compared
    });
    println!("  {CASES} searches and {creates} creates compared");
}
