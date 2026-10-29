//! `is_on_curve` against curve25519-dalek's decompression, the test
//! solana-pubkey runs, on edge encodings, real points and random bytes.

mod common;

use common::{dalek_on_curve, Rng};
use curve25519_dalek::{constants, scalar::Scalar};
use fastpda::is_on_curve;
use sha2::{Digest, Sha256};

/// `y` with the top (sign) bit clear and set.
fn both_signs(y: [u8; 32]) -> [[u8; 32]; 2] {
    let mut clear = y;
    clear[31] &= 0x7f;
    let mut set = y;
    set[31] |= 0x80;
    [clear, set]
}

fn from_u64(x: u64) -> [u8; 32] {
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&x.to_le_bytes());
    b
}

/// p - k for 1 <= k <= 0xed: the low byte of p is 0xed, the rest 0xff (top 0x7f).
fn p_minus(k: u8) -> [u8; 32] {
    let mut b = [0xff; 32];
    b[0] = 0xed - k;
    b[31] = 0x7f;
    b
}

/// p + k for 0 <= k <= 18: the 19 non-canonical encodings below 2^255.
fn p_plus(k: u8) -> [u8; 32] {
    let mut b = [0xff; 32];
    b[0] = 0xed + k;
    b[31] = 0x7f;
    b
}

fn check(bytes: &[u8; 32]) -> bool {
    let expected = dalek_on_curve(bytes);
    assert_eq!(is_on_curve(bytes), expected, "bytes {bytes:02x?}");
    expected
}

#[test]
fn edge_encodings_match_dalek() {
    let mut cases = Vec::new();
    // Small y: y = 0 (x^2 = -1), y = 1 (the identity, u = 0), ...
    for y in 0..256u64 {
        cases.extend(both_signs(from_u64(y)));
    }
    // y = p - k: -1 (the point of order 2, u = 0), -2, ...
    for k in 1..=0xed {
        cases.extend(both_signs(p_minus(k)));
    }
    // y = p .. 2^255 - 1: dalek reads these as y - p instead of refusing
    // them, so their answers must equal those of 0 .. 18.
    for k in 0..=18 {
        let non_canonical = both_signs(p_plus(k));
        let canonical = both_signs(from_u64(k as u64));
        for (a, b) in non_canonical.iter().zip(&canonical) {
            assert_eq!(check(a), check(b), "p + {k}");
        }
    }
    // The eight small-order points, and the identity and order-2 point with
    // the sign bit set ("negative zero", which RFC 8032 rejects and dalek
    // accepts).
    for point in constants::EIGHT_TORSION {
        cases.extend(both_signs(point.compress().to_bytes()));
    }
    // Every single bit set, and every single bit clear.
    for bit in 0..256 {
        let mut b = [0u8; 32];
        b[bit / 8] |= 1 << (bit % 8);
        cases.push(b);
        cases.push(b.map(|x| !x));
    }
    // Repeated bytes.
    for byte in 0..=255u8 {
        cases.push([byte; 32]);
    }
    let on = cases.iter().filter(|c| check(c)).count();
    // The list covers both answers.
    assert!(
        on > 0 && on < cases.len(),
        "{on} of {} on the curve",
        cases.len()
    );
}

#[test]
fn negative_zero_is_on_the_curve() {
    // y = 1 with the sign bit set: x = 0 but marked negative.
    let mut b = from_u64(1);
    b[31] |= 0x80;
    assert!(dalek_on_curve(&b));
    assert!(is_on_curve(&b));
}

#[test]
fn real_points_are_on_the_curve() {
    let mut rng = Rng::new(0x5ea5_0002);
    for _ in 0..2_000 {
        let mut wide = [0u8; 64];
        rng.fill(&mut wide);
        let point = constants::ED25519_BASEPOINT_POINT * Scalar::from_bytes_mod_order_wide(&wide);
        // P and -P differ only in the sign bit.
        for bytes in both_signs(point.compress().to_bytes()) {
            assert!(is_on_curve(&bytes), "bytes {bytes:02x?}");
        }
        // And small-order components do not change that.
        let torsion = constants::EIGHT_TORSION[rng.below(8)];
        assert!(is_on_curve(&(point + torsion).compress().to_bytes()));
    }
}

#[test]
fn random_bytes_match_dalek() {
    let mut rng = Rng::new(0x5ea5_0003);
    let mut on = 0;
    for _ in 0..200_000 {
        on += check(&rng.bytes32()) as u32;
    }
    // About half of all 32-byte strings decode to a point.
    assert!((98_000..102_000).contains(&on), "{on} points in 200,000");
}

#[test]
fn sha256_outputs_match_dalek() {
    // What a PDA try feeds the test.
    for i in 0..200_000u64 {
        check(&Sha256::digest(i.to_le_bytes()).into());
    }
}
