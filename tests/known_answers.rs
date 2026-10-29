//! Known-answer vectors from reference/pda_reference.py, an independent
//! pure-Python implementation (hashlib SHA-256, big-integer field arithmetic,
//! on-curve test by full point decompression). Each vector is checked against
//! fastpda and, as a cross-check of the file itself, against solana-pubkey
//! and curve25519-dalek.

mod common;

use common::{dalek_on_curve, hex, hex32, solana_create, solana_try_find};
use fastpda::{bytes, is_on_curve, Error};

const VECTORS: &str = include_str!("../reference/vectors.txt");

fn seeds(field: &str) -> Vec<Vec<u8>> {
    let (count, rest) = field.split_once(':').expect("count:seeds");
    let count: usize = count.parse().expect("seed count");
    let seeds: Vec<Vec<u8>> = if count == 0 {
        Vec::new()
    } else {
        rest.split(',').map(hex).collect()
    };
    assert_eq!(seeds.len(), count, "{field}");
    seeds
}

#[test]
fn known_answers_match() {
    let (mut curve, mut create, mut find, mut low_bumps) = (0, 0, 0, 0);
    for line in VECTORS
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let fields: Vec<&str> = line.split(' ').collect();
        match fields[0] {
            "curve" => {
                let bytes = hex32(fields[1]);
                let expected = fields[2] == "1";
                assert_eq!(is_on_curve(&bytes), expected, "{line}");
                assert_eq!(
                    dalek_on_curve(&bytes),
                    expected,
                    "dalek disagrees with the reference: {line}"
                );
                curve += 1;
            }
            "create" => {
                let program_id = hex32(fields[1]);
                let seeds = seeds(fields[2]);
                let seeds: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
                let expected = match fields[3] {
                    "on-curve" => Err(Error::InvalidSeeds),
                    "too-long" => Err(Error::MaxSeedLengthExceeded),
                    address => Ok(hex32(address)),
                };
                assert_eq!(
                    bytes::create_program_address(&seeds, &program_id),
                    expected,
                    "{line}"
                );
                assert_eq!(
                    solana_create(&seeds, &program_id),
                    expected,
                    "solana disagrees with the reference: {line}"
                );
                create += 1;
            }
            "find" => {
                let program_id = hex32(fields[1]);
                let seeds = seeds(fields[2]);
                let seeds: Vec<&[u8]> = seeds.iter().map(|s| s.as_slice()).collect();
                let expected = match fields[3] {
                    "none" => None,
                    address => Some((hex32(address), fields[4].parse::<u8>().expect("bump"))),
                };
                assert_eq!(
                    bytes::try_find_program_address(&seeds, &program_id),
                    expected,
                    "{line}"
                );
                assert_eq!(
                    solana_try_find(&seeds, &program_id),
                    expected,
                    "solana disagrees with the reference: {line}"
                );
                find += 1;
                low_bumps += expected.is_some_and(|(_, bump)| bump < 240) as u32;
            }
            "square" => {} // checked by the unit tests in src/tests/jacobi.rs
            other => panic!("unknown vector kind {other:?}"),
        }
    }
    // The file is all there.
    assert!(
        curve >= 2_800 && create >= 300 && find >= 500,
        "{curve} curve, {create} create, {find} find"
    );
    assert!(low_bumps >= 8, "{low_bumps} finds below bump 240");
}
