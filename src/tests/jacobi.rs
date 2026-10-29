//! `is_square` against Euler's criterion (num-bigint's modpow), plus the
//! structure every correct answer has.

use num_bigint::BigUint;
use proptest::prelude::*;

use super::*;
use crate::test_util::{euler_is_square, from_le, p, to_le, Rng};

fn small(x: u64) -> [u8; 32] {
    to_le(&BigUint::from(x))
}

fn check(bytes: &[u8; 32]) {
    assert_eq!(
        is_square(bytes),
        euler_is_square(&from_le(bytes)),
        "a = {bytes:02x?}"
    );
}

#[test]
fn small_numbers_have_the_known_symbols() {
    // p = 5 mod 8, so 2 is not a square; p = 1 mod 4, so -1 is.
    assert!(is_square(&small(0)));
    assert!(is_square(&small(1)));
    assert!(!is_square(&small(2)));
    assert!(is_square(&small(4)));
    assert!(is_square(&to_le(&(p() - 1u32))));
    for x in 0..3_000u64 {
        check(&small(x));
    }
}

#[test]
fn numbers_near_p_and_past_it_reduce_mod_p() {
    // p - k, p + k up to 2^255 - 1 (the non-canonical range), and on to 2^256 - 1.
    for k in 0..64u32 {
        check(&to_le(&(p() - (k + 1))));
        check(&to_le(&(p() + k)));
        // 2p + k stays below 2^256 = 2p + 38 for k < 38.
        if k < 38 {
            check(&to_le(&((p() << 1u32) + k)));
        }
        check(&to_le(&((BigUint::from(1u8) << 256u32) - (k + 1))));
    }
    // Multiples of p have symbol 0, which counts as a square.
    for m in 0..=2u32 {
        assert!(is_square(&to_le(&(p() * m))), "{m} * p");
    }
    // 2^256 - 1 = 2p + 37.
    assert_eq!(is_square(&[0xff; 32]), is_square(&small(37)));
}

#[test]
fn every_shift_length_is_exercised() {
    // 2^j strips j zero bits before the first pass; p + 2^j and 2p + 2^j
    // (both 2^j mod p) strip j after a subtraction. Together they drive every
    // shift from 1 to 255, including the ones across the 128-bit halves.
    // (2 | p) = -1, so 2^j is a square exactly for even j.
    for j in 0..256u32 {
        let power = BigUint::from(1u8) << j;
        assert_eq!(is_square(&to_le(&power)), j % 2 == 0, "2^{j}");
        for m in [1u32, 2] {
            let x = p() * m + &power;
            if x.bits() <= 256 {
                assert_eq!(is_square(&to_le(&x)), j % 2 == 0, "{m}p + 2^{j}");
            }
        }
        // An odd number with a long run of zeros above its lowest bit, and
        // two set bits at the ends of a gap.
        check(&to_le(&((BigUint::from(1u8) << j) + 1u32)));
        check(&to_le(&(BigUint::from(3u8) << j.min(254))));
    }
}

#[test]
fn shift_and_trailing_zeros_match_big_integers() {
    let mut rng = Rng::new(0x5ea5_0011);
    for _ in 0..200 {
        let bytes = rng.bytes32();
        let lo = u128::from_le_bytes(bytes[..16].try_into().unwrap());
        let hi = u128::from_le_bytes(bytes[16..].try_into().unwrap());
        let x = from_le(&bytes);
        for k in 1..=255u32 {
            let (h, l) = shift_right((hi, lo), k);
            let mut out = [0u8; 32];
            out[..16].copy_from_slice(&l.to_le_bytes());
            out[16..].copy_from_slice(&h.to_le_bytes());
            assert_eq!(from_le(&out), &x >> k, "x >> {k}");
        }
        for j in 0..256u32 {
            // x with its low j bits cleared and bit j set has j trailing zeros.
            let y = ((&x >> j) | BigUint::from(1u8)) << j;
            if y.bits() <= 256 {
                let b = to_le(&y);
                let lo = u128::from_le_bytes(b[..16].try_into().unwrap());
                let hi = u128::from_le_bytes(b[16..].try_into().unwrap());
                assert_eq!(trailing_zeros((hi, lo)), j);
            }
        }
    }
}

#[test]
fn random_numbers_match_euler() {
    let mut rng = Rng::new(0x5ea5_0001);
    let mut squares = 0;
    for i in 0..50_000 {
        let mut a = rng.bytes32();
        // Half the cases below 2^255 (field-sized), half full 256-bit.
        if i % 2 == 0 {
            a[31] &= 0x7f;
        }
        let expected = euler_is_square(&from_le(&a));
        assert_eq!(is_square(&a), expected, "a = {a:02x?}");
        squares += expected as u32;
    }
    // About half of all residues are squares: a broken reference would show here.
    assert!(
        (24_000..26_000).contains(&squares),
        "{squares} squares in 50,000"
    );
}

#[test]
fn squares_are_squares_and_twice_a_square_is_not() {
    // Independent of Euler: x^2 is a square by construction, and since 2 is
    // not a square, 2x^2 is not one (for x not a multiple of p).
    let mut rng = Rng::new(0x5ea5_0012);
    for _ in 0..2_000 {
        let x = from_le(&rng.bytes32()) % p();
        if x == BigUint::from(0u8) {
            continue;
        }
        let square = (&x * &x) % p();
        assert!(is_square(&to_le(&square)));
        assert!(!is_square(&to_le(&((square * 2u32) % p()))));
    }
}

proptest! {
    #[test]
    fn matches_euler_on_any_256_bit_input(a in any::<[u8; 32]>()) {
        prop_assert_eq!(is_square(&a), euler_is_square(&from_le(&a)));
    }

    #[test]
    fn the_symbol_is_multiplicative(a in any::<[u8; 32]>(), b in any::<[u8; 32]>()) {
        // (ab | p) = (a | p)(b | p): for nonzero a and b mod p, ab is a square
        // exactly when a and b are both squares or both not.
        let (x, y) = (from_le(&a) % p(), from_le(&b) % p());
        prop_assume!(x != BigUint::from(0u8) && y != BigUint::from(0u8));
        let product = to_le(&((&x * &y) % p()));
        prop_assert_eq!(is_square(&product), is_square(&a) == is_square(&b));
    }

    #[test]
    fn adding_p_changes_nothing(a in any::<[u8; 32]>()) {
        let x = from_le(&a) % p();
        let symbol = is_square(&to_le(&x));
        prop_assert_eq!(is_square(&to_le(&(&x + p()))), symbol);
        prop_assert_eq!(is_square(&a), symbol);
    }
}

#[test]
fn known_answers_from_the_python_reference_match() {
    // The "square" lines of reference/vectors.txt: Euler's criterion computed
    // by Python's pow() on big integers.
    let mut checked = 0;
    for line in include_str!("../../reference/vectors.txt").lines() {
        let Some(rest) = line.strip_prefix("square ") else {
            continue;
        };
        let (hex, expected) = rest.split_once(' ').unwrap();
        let mut a = [0u8; 32];
        for (i, byte) in a.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
        }
        assert_eq!(is_square(&a), expected == "1", "{line}");
        checked += 1;
    }
    assert!(checked >= 1_800, "{checked} vectors");
}

/// The long run, with the other sweeps (tests/sweeps.rs):
/// `cargo test --release -- --ignored sweep --nocapture --test-threads=1`.
/// 10 million random 256-bit numbers against Euler's criterion, on every core.
#[test]
#[ignore]
fn sweep_random_numbers_match_euler() {
    use std::sync::atomic::{AtomicU64, Ordering};
    const CASES: u64 = 10_000_000;
    const CHUNK: u64 = 100_000;
    let seed = crate::test_util::run_seed("sweep_random_numbers_match_euler");
    let start = std::time::Instant::now();
    let next_chunk = AtomicU64::new(0);
    let squares = AtomicU64::new(0);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let chunk = next_chunk.fetch_add(1, Ordering::Relaxed);
                if chunk * CHUNK >= CASES {
                    break;
                }
                let mut rng = Rng::new(seed ^ chunk.wrapping_mul(0xd1b5_4a32_d192_ed03));
                let mut count = 0;
                for _ in 0..CHUNK {
                    let a = rng.bytes32();
                    let expected = euler_is_square(&from_le(&a));
                    assert_eq!(is_square(&a), expected, "a = {a:02x?}, seed {seed:#x}");
                    count += expected as u64;
                }
                squares.fetch_add(count, Ordering::Relaxed);
            });
        }
    });
    let squares = squares.into_inner();
    println!(
        "  {CASES} numbers, {squares} squares, {threads} threads, {:.1} s",
        start.elapsed().as_secs_f64()
    );
    assert!(squares > CASES * 2 / 5 && squares < CASES * 3 / 5);
}
