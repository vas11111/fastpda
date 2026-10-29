//! The field side of the on-curve test: the constant d, the facts the test
//! relies on, and u*v against big-integer arithmetic. (The on-curve answers
//! themselves are compared with curve25519-dalek in tests/dalek.rs.)

use num_bigint::BigUint;
use proptest::prelude::*;

use super::*;
use crate::test_util::{euler_is_square, from_le, p, to_le, Rng};

fn d() -> BigUint {
    D.0.iter()
        .rev()
        .fold(BigUint::from(0u8), |acc, limb| (acc << 51u32) + *limb)
}

/// u*v = (y^2 - 1)(d*y^2 + 1) mod p, with y read as dalek reads it.
fn u_times_v_reference(bytes: &[u8; 32]) -> BigUint {
    let p = p();
    let mut y_bytes = *bytes;
    y_bytes[31] &= 0x7f;
    let y = from_le(&y_bytes) % &p;
    let yy = (&y * &y) % &p;
    let u = (&yy + &p - 1u32) % &p;
    let v = (d() * &yy + 1u32) % &p;
    (u * v) % &p
}

#[test]
fn d_is_minus_121665_over_121666() {
    let p = p();
    assert!(d() < p);
    assert_eq!((d() * 121_666u32 + 121_665u32) % &p, BigUint::from(0u8));
}

#[test]
fn v_is_never_zero() {
    // v = d*y^2 + 1 = 0 would need y^2 = -1/d. -1 is a square and d is not,
    // so -1/d is not a square and no y^2 equals it.
    let p = p();
    let minus_one = &p - 1u32;
    assert!(euler_is_square(&minus_one), "-1 is a square");
    assert!(!euler_is_square(&d()), "d is not a square");
    let inverse_d = d().modpow(&(&p - 2u32), &p);
    assert!(
        !euler_is_square(&((minus_one * inverse_d) % &p)),
        "-1/d is not a square"
    );
    // And the crate's own Jacobi symbol agrees on both.
    assert!(jacobi::is_square(&to_le(&(&p - 1u32))));
    assert!(!jacobi::is_square(&to_le(&d())));
}

#[test]
fn u_times_v_matches_big_integers_on_edge_values() {
    let p = p();
    let mut cases: Vec<[u8; 32]> = Vec::new();
    for k in 0..64u32 {
        cases.push(to_le(&BigUint::from(k)));
        cases.push(to_le(&(&p - (k + 1))));
    }
    // The non-canonical y: p .. 2^255 - 1.
    for k in 0..19u32 {
        cases.push(to_le(&(&p + k)));
    }
    for bit in 0..256 {
        let mut b = [0u8; 32];
        b[bit / 8] |= 1 << (bit % 8);
        cases.push(b);
        cases.push(b.map(|x| !x));
    }
    cases.push([0xff; 32]);
    let with_sign: Vec<[u8; 32]> = cases
        .iter()
        .map(|c| {
            let mut c = *c;
            c[31] ^= 0x80;
            c
        })
        .collect();
    for case in cases.iter().chain(&with_sign) {
        assert_eq!(
            from_le(&u_times_v(case)),
            u_times_v_reference(case),
            "y = {case:02x?}"
        );
    }
}

#[test]
fn u_times_v_matches_big_integers_on_random_values() {
    let mut rng = Rng::new(0x5ea5_0013);
    for _ in 0..100_000 {
        let y = rng.bytes32();
        assert_eq!(
            from_le(&u_times_v(&y)),
            u_times_v_reference(&y),
            "y = {y:02x?}"
        );
    }
}

#[test]
fn y_and_y_plus_p_and_both_signs_agree() {
    let p = p();
    for k in 0..19u32 {
        let canonical = to_le(&BigUint::from(k));
        let non_canonical = to_le(&(&p + k));
        let expected = is_on_curve(&canonical);
        for mut bytes in [canonical, non_canonical] {
            assert_eq!(is_on_curve(&bytes), expected, "y = {k} (+p)");
            bytes[31] ^= 0x80;
            assert_eq!(is_on_curve(&bytes), expected, "y = {k} (+p), sign bit");
        }
    }
}

proptest! {
    #[test]
    fn u_times_v_matches_big_integers(y in any::<[u8; 32]>()) {
        prop_assert_eq!(from_le(&u_times_v(&y)), u_times_v_reference(&y));
    }

    #[test]
    fn on_curve_is_euler_on_u_times_v(y in any::<[u8; 32]>()) {
        prop_assert_eq!(is_on_curve(&y), euler_is_square(&u_times_v_reference(&y)));
    }
}
