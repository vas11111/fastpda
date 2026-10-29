//! The ed25519 on-curve test a PDA must fail.
//!
//! # The math
//!
//! Ed25519 is the twisted Edwards curve `-x^2 + y^2 = 1 + d*x^2*y^2` over the
//! field of integers mod p = 2^255 - 19, with d = -121665/121666 mod p. A point
//! is stored as 32 bytes: y in the low 255 bits (little-endian) and the sign
//! (lowest bit) of x in the top bit. Decompressing means recovering x, and
//! solving the curve equation for x^2 gives
//!
//! ```text
//! x^2 = u / v,   u = y^2 - 1,   v = d*y^2 + 1
//! ```
//!
//! so some point has this y exactly when u/v has a square root mod p.
//!
//! - **v is never 0.** v = 0 would need y^2 = -1/d. -1 is a square mod p
//!   (p = 1 mod 4) and d is not (both are checked in the tests), so -1/d is
//!   not a square and no y has y^2 = -1/d.
//! - **u/v is a square exactly when u*v is.** u*v = (u/v) * v^2, and v^2 is a
//!   nonzero square, so the two have the same Legendre symbol. Using u*v
//!   avoids the field inversion.
//! - **u = 0 is a point.** y = 1 and y = -1 give x = 0 (the identity and the
//!   point of order 2). u*v is 0 there, and 0 counts as a square.
//!
//! So: bytes are on the curve exactly when u*v mod p is 0 or a square, which
//! is the Jacobi symbol `(u*v | p) != -1`. That is three field operations
//! (fiat-crypto) and [`jacobi::is_square`], where solana-pubkey (through
//! curve25519-dalek's `decompress`) computes the square root itself, a field
//! exponentiation of about 254 squarings.
//!
//! # Which encodings count
//!
//! The answer must match `CompressedEdwardsY::decompress().is_some()` from
//! curve25519-dalek, which solana-pubkey runs, on all 2^256 inputs, including
//! the encodings RFC 8032 would reject:
//!
//! - **The sign bit is ignored.** If x is a root so is -x, so the sign never
//!   decides existence. RFC 8032 rejects the encoding of x = 0 with the sign
//!   bit set ("negative zero"); dalek accepts it (negating 0 gives 0), so
//!   that is on the curve here too.
//! - **y >= p is read as y - p.** The 19 values p .. 2^255 - 1 are
//!   non-canonical encodings of 0 .. 18. RFC 8032 rejects them; dalek's
//!   `FieldElement::from_bytes` reduces them instead, and so does fiat's
//!   `from_bytes` here: the field operations work mod p, and the product is
//!   fully reduced before the Jacobi symbol reads it.

use fiat_crypto::curve25519_64::{
    fiat_25519_add, fiat_25519_carry_mul, fiat_25519_carry_square, fiat_25519_from_bytes,
    fiat_25519_loose_field_element as Loose, fiat_25519_relax, fiat_25519_sub,
    fiat_25519_tight_field_element as Tight, fiat_25519_to_bytes,
};

use crate::jacobi;

/// d = -121665/121666 mod p as five 51-bit limbs, least significant first
/// (the same limbs as curve25519-dalek's `EDWARDS_D`; a test checks that
/// 121666 * d + 121665 = 0 mod p).
pub(crate) const D: Tight = Tight([
    929955233495203,
    466365720129213,
    1662059464998953,
    2033849074728123,
    1442794654840575,
]);

const ONE: Tight = Tight([1, 0, 0, 0, 0]);

/// Whether `bytes` is the encoding of an ed25519 point: the same answer as
/// curve25519-dalek's `CompressedEdwardsY(bytes).decompress().is_some()`,
/// which is solana-pubkey's `Pubkey::is_on_curve` / `bytes_are_curve_point`.
/// A PDA is valid only when this is `false` for it.
///
/// # How
///
/// The bytes hold y (low 255 bits) and the sign of x (top bit). The curve
/// equation `-x^2 + y^2 = 1 + d*x^2*y^2` gives `x^2 = u/v` with
/// `u = y^2 - 1` and `v = d*y^2 + 1`, so a point with this y exists exactly
/// when u/v is a square mod p = 2^255 - 19. v is never 0 (that needs
/// `y^2 = -1/d`, which is not a square), and u/v has the same quadratic
/// character as `u*v = (u/v) * v^2`. So the answer is whether `u*v` is 0 or a
/// square: three field multiplications and one Jacobi symbol, computed by the
/// binary (gcd-like) algorithm, instead of the square root (a ~254-squaring
/// exponentiation) that decompression computes.
///
/// # Which encodings count
///
/// Exactly dalek's, including encodings RFC 8032 rejects: the sign bit is
/// ignored (a root's negation is a root; "negative zero" for x = 0 is
/// accepted), and the 19 non-canonical values y = p .. 2^255 - 1 are read as
/// y - p rather than refused.
///
/// # Timing
///
/// Variable time: do not call it on secret data. PDAs are public.
///
/// ```
/// // y = 1 is the identity point; y = 2 is not a point.
/// let mut y = [0u8; 32];
/// y[0] = 1;
/// assert!(fastpda::is_on_curve(&y));
/// y[0] = 2;
/// assert!(!fastpda::is_on_curve(&y));
/// ```
pub fn is_on_curve(bytes: &[u8; 32]) -> bool {
    jacobi::is_square(&u_times_v(bytes))
}

/// u*v = (y^2 - 1)(d*y^2 + 1) mod p for the y in `bytes`, as 32 canonical
/// little-endian bytes (below p).
pub(crate) fn u_times_v(bytes: &[u8; 32]) -> [u8; 32] {
    // The top bit is the sign of x, not part of y. fiat's from_bytes requires
    // it clear, and accepts any y below 2^255, canonical or not.
    let mut y_bytes = *bytes;
    y_bytes[31] &= 0x7f;

    // fiat's functions write into their first argument. Results are "tight"
    // elements; multiplications take "loose" ones, and `relax` converts. The
    // bounds are fiat's, and each call below meets them by construction.
    let mut y = Tight([0; 5]);
    fiat_25519_from_bytes(&mut y, &y_bytes);
    let mut y_loose = Loose([0; 5]);
    fiat_25519_relax(&mut y_loose, &y);

    let mut yy = Tight([0; 5]);
    fiat_25519_carry_square(&mut yy, &y_loose);
    let mut yy_loose = Loose([0; 5]);
    fiat_25519_relax(&mut yy_loose, &yy);

    // u = y^2 - 1
    let mut u = Loose([0; 5]);
    fiat_25519_sub(&mut u, &yy, &ONE);

    // v = d*y^2 + 1
    let mut d_loose = Loose([0; 5]);
    fiat_25519_relax(&mut d_loose, &D);
    let mut dyy = Tight([0; 5]);
    fiat_25519_carry_mul(&mut dyy, &d_loose, &yy_loose);
    let mut v = Loose([0; 5]);
    fiat_25519_add(&mut v, &dyy, &ONE);

    let mut uv = Tight([0; 5]);
    fiat_25519_carry_mul(&mut uv, &u, &v);
    // to_bytes reduces fully: the output is the canonical value below p.
    let mut out = [0u8; 32];
    fiat_25519_to_bytes(&mut out, &uv);
    out
}

#[cfg(test)]
#[path = "tests/curve.rs"]
mod tests;
