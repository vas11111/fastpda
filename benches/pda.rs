//! fastpda against solana-pubkey, per call:
//!
//! - `on_curve`: the on-curve test alone, on SHA-256 outputs (what a PDA try
//!   feeds it): curve25519-dalek's decompression vs the Jacobi symbol;
//! - `create_program_address`: one try (one hash and one on-curve test);
//! - `find_program_address`: the whole search, for several seed shapes;
//! - `find_by_tries`: the search on inputs whose first off-curve bump is
//!   found at try 1, 2, ..., 8, so the cost per try shows.
//!
//! Before the timings it prints how many tries searches take on random
//! inputs. Run with `cargo bench` (or `bench/run.sh`, which also writes a CSV).
//!
//! Each benchmark cycles through a pool of distinct random inputs, so neither
//! side gets a branch predictor or cache trained on one input. Both sides hash
//! with the same hardware SHA-256 (see Cargo.toml), so the difference is the
//! on-curve test and the hashing layout.

use std::{hint::black_box, time::Duration};

use criterion::{BenchmarkId, Criterion};
use sha2::{Digest, Sha256};
use solana_pubkey::{pubkey, Pubkey};

const POOL: usize = 1024;

const TOKEN_PROGRAM: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ASSOCIATED_TOKEN_PROGRAM: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// SplitMix64, as in the tests: the same inputs on every run and machine.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next_u64() as u8).collect()
    }

    fn key(&mut self) -> Pubkey {
        Pubkey::new_from_array(self.bytes(32).try_into().unwrap())
    }
}

/// A seed-list shape: a name, and how to make one random instance of it
/// along with the program id it is derived under.
struct Shape {
    name: &'static str,
    make: fn(&mut Rng) -> (Vec<Vec<u8>>, Pubkey),
}

fn uniform<const COUNT: usize, const LEN: usize>(rng: &mut Rng) -> (Vec<Vec<u8>>, Pubkey) {
    let program_id = rng.key();
    ((0..COUNT).map(|_| rng.bytes(LEN)).collect(), program_id)
}

/// `[owner, token program, mint]` under the associated token program.
fn ata_like(rng: &mut Rng) -> (Vec<Vec<u8>>, Pubkey) {
    let (owner, mint) = (rng.key(), rng.key());
    (
        vec![
            owner.to_bytes().to_vec(),
            TOKEN_PROGRAM.to_bytes().to_vec(),
            mint.to_bytes().to_vec(),
        ],
        ASSOCIATED_TOKEN_PROGRAM,
    )
}

/// `["bonding-curve", mint]`: a short string and a key.
fn pump_curve_like(rng: &mut Rng) -> (Vec<Vec<u8>>, Pubkey) {
    let program_id = rng.key();
    (vec![b"bonding-curve".to_vec(), rng.bytes(32)], program_id)
}

/// The shapes `find` can take (at most 15 seeds: the bump is the 16th).
const FIND_SHAPES: &[Shape] = &[
    Shape {
        name: "1x8B",
        make: uniform::<1, 8>,
    },
    Shape {
        name: "1x32B",
        make: uniform::<1, 32>,
    },
    Shape {
        name: "pump-curve-like",
        make: pump_curve_like,
    },
    Shape {
        name: "ata-like",
        make: ata_like,
    },
    Shape {
        name: "4x8B",
        make: uniform::<4, 8>,
    },
    Shape {
        name: "4x32B",
        make: uniform::<4, 32>,
    },
    Shape {
        name: "8x32B",
        make: uniform::<8, 32>,
    },
    Shape {
        name: "15x8B",
        make: uniform::<15, 8>,
    },
    Shape {
        name: "15x32B",
        make: uniform::<15, 32>,
    },
];

/// Shapes only `create` takes: 16 seeds, the most a PDA can have.
const CREATE_ONLY_SHAPES: &[Shape] = &[
    Shape {
        name: "16x8B",
        make: uniform::<16, 8>,
    },
    Shape {
        name: "16x32B",
        make: uniform::<16, 32>,
    },
];

fn pool(shape: &Shape, rng: &mut Rng) -> Vec<(Vec<Vec<u8>>, Pubkey)> {
    (0..POOL).map(|_| (shape.make)(rng)).collect()
}

fn slices(seeds: &[Vec<u8>]) -> Vec<&[u8]> {
    seeds.iter().map(|s| s.as_slice()).collect()
}

/// Times `f` over `inputs`, one input per iteration, cycling.
fn cycle<T>(b: &mut criterion::Bencher<'_>, inputs: &[T], mut f: impl FnMut(&T)) {
    let mut i = 0;
    b.iter(|| {
        f(black_box(&inputs[i]));
        i = (i + 1) % inputs.len();
    });
}

fn on_curve(c: &mut Criterion) {
    let hashes: Vec<[u8; 32]> = (0..POOL as u64)
        .map(|i| Sha256::digest(i.to_le_bytes()).into())
        .collect();
    let mut group = c.benchmark_group("on_curve");
    group.bench_function("solana-pubkey", |b| {
        cycle(b, &hashes, |h| {
            black_box(solana_pubkey::bytes_are_curve_point(h));
        })
    });
    group.bench_function("fastpda", |b| {
        cycle(b, &hashes, |h| {
            black_box(fastpda::is_on_curve(h));
        })
    });
    group.finish();
}

fn create(c: &mut Criterion) {
    let mut rng = Rng(0xbe4c_0001);
    let mut group = c.benchmark_group("create_program_address");
    for shape in FIND_SHAPES.iter().chain(CREATE_ONLY_SHAPES) {
        // The find shapes get a random bump as their last seed, as when a
        // program checks a stored bump.
        let inputs: Vec<(Vec<Vec<u8>>, Pubkey)> = pool(shape, &mut rng)
            .into_iter()
            .map(|(mut seeds, program_id)| {
                if seeds.len() < 16 {
                    seeds.push(vec![rng.next_u64() as u8]);
                }
                (seeds, program_id)
            })
            .collect();
        let inputs: Vec<(Vec<&[u8]>, Pubkey)> =
            inputs.iter().map(|(s, p)| (slices(s), *p)).collect();
        group.bench_with_input(
            BenchmarkId::new("solana-pubkey", shape.name),
            &inputs,
            |b, inputs| {
                cycle(b, inputs, |(seeds, program_id)| {
                    black_box(Pubkey::create_program_address(seeds, program_id).ok());
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("fastpda", shape.name),
            &inputs,
            |b, inputs| {
                cycle(b, inputs, |(seeds, program_id)| {
                    black_box(fastpda::create_program_address(seeds, program_id).ok());
                })
            },
        );
    }
    group.finish();
}

fn find(c: &mut Criterion) {
    let mut rng = Rng(0xbe4c_0002);
    let mut group = c.benchmark_group("find_program_address");
    for shape in FIND_SHAPES {
        let inputs = pool(shape, &mut rng);
        let inputs: Vec<(Vec<&[u8]>, Pubkey)> =
            inputs.iter().map(|(s, p)| (slices(s), *p)).collect();
        group.bench_with_input(
            BenchmarkId::new("solana-pubkey", shape.name),
            &inputs,
            |b, inputs| {
                cycle(b, inputs, |(seeds, program_id)| {
                    black_box(Pubkey::find_program_address(seeds, program_id));
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("fastpda", shape.name),
            &inputs,
            |b, inputs| {
                cycle(b, inputs, |(seeds, program_id)| {
                    black_box(fastpda::find_program_address(seeds, program_id));
                })
            },
        );
    }
    group.finish();
}

/// ATA-like inputs whose search ends at try `tries` (bump 256 - tries).
fn inputs_with_tries(rng: &mut Rng, tries: u8, count: usize) -> Vec<(Vec<Vec<u8>>, Pubkey)> {
    let mut out = Vec::with_capacity(count);
    while out.len() < count {
        let (seeds, program_id) = ata_like(rng);
        let (_, bump) = fastpda::find_program_address(&slices(&seeds), &program_id);
        if bump == u8::MAX - (tries - 1) {
            out.push((seeds, program_id));
        }
    }
    out
}

fn find_by_tries(c: &mut Criterion) {
    let mut rng = Rng(0xbe4c_0003);
    let mut group = c.benchmark_group("find_by_tries");
    for tries in 1..=8u8 {
        let inputs = inputs_with_tries(&mut rng, tries, 256);
        let inputs: Vec<(Vec<&[u8]>, Pubkey)> =
            inputs.iter().map(|(s, p)| (slices(s), *p)).collect();
        let case = format!("{tries}");
        group.bench_with_input(
            BenchmarkId::new("solana-pubkey", &case),
            &inputs,
            |b, inputs| {
                cycle(b, inputs, |(seeds, program_id)| {
                    black_box(Pubkey::find_program_address(seeds, program_id));
                })
            },
        );
        group.bench_with_input(BenchmarkId::new("fastpda", &case), &inputs, |b, inputs| {
            cycle(b, inputs, |(seeds, program_id)| {
                black_box(fastpda::find_program_address(seeds, program_id));
            })
        });
    }
    group.finish();
}

/// How many tries searches take on random ATA-like inputs. Each try is off
/// the curve with probability about 1/2, so about half the searches end at
/// try 1, a quarter at try 2, and the mean is about 2.
fn print_tries_distribution() {
    const SEARCHES: usize = 1_000_000;
    let mut rng = Rng(0xbe4c_0004);
    let mut counts = [0usize; 256];
    for _ in 0..SEARCHES {
        let (seeds, program_id) = ata_like(&mut rng);
        let (_, bump) = fastpda::find_program_address(&slices(&seeds), &program_id);
        counts[(u8::MAX - bump) as usize + 1] += 1;
    }
    let mean = counts
        .iter()
        .enumerate()
        .map(|(tries, n)| tries * n)
        .sum::<usize>() as f64
        / SEARCHES as f64;
    println!("tries per search over {SEARCHES} random ATA-like searches (mean {mean:.4}):");
    for (tries, n) in counts.iter().enumerate().filter(|(_, n)| **n > 0) {
        println!(
            "  {tries:>2} tries: {n:>7}  {:6.3}%",
            100.0 * *n as f64 / SEARCHES as f64
        );
    }
}

fn main() {
    let mut c = Criterion::default()
        .sample_size(50)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
        .configure_from_args();
    if !std::env::args().any(|a| a == "--list") {
        print_tries_distribution();
    }
    on_curve(&mut c);
    create(&mut c);
    find(&mut c);
    find_by_tries(&mut c);
    c.final_summary();
}
