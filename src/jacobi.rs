//! Whether a number is a square mod p = 2^255 - 19, from its Jacobi symbol.
//!
//! # The symbol
//!
//! For a prime p, the Legendre symbol (a | p) is 0 when p divides a, 1 when a
//! is a nonzero square mod p, and -1 otherwise. The textbook way to compute it
//! is Euler's criterion, a^((p-1)/2) mod p: about 254 squarings and a dozen
//! multiplications of 255-bit numbers.
//!
//! The Jacobi symbol (a | n) extends it to every odd n > 0, and obeys rules
//! that let it be computed the way a gcd is, by shrinking the two numbers:
//!
//! 1. it depends only on a mod n: (a | n) = (a - n | n);
//! 2. factors of 2 come out one at a time: (2a | n) = (2 | n) * (a | n), and
//!    (2 | n) is -1 exactly when n is 3 or 5 mod 8;
//! 3. quadratic reciprocity: for odd a and n, (a | n) = (n | a), negated when
//!    a and n are both 3 mod 4 (when they share a factor both sides are 0,
//!    so the rule still holds);
//! 4. (0 | 1) = 1, and (0 | n) = 0 for n > 1.
//!
//! # The binary algorithm
//!
//! Start with (a | p). Remove a's factors of 2 (rule 2). Then, while a is not
//! 0, with a and n both odd:
//!
//! - if a < n, swap them (rule 3), so that a >= n;
//! - replace a by a - n (rule 1), which is even (odd minus odd);
//! - if a is now 0, stop; otherwise remove its factors of 2 (rule 2).
//!
//! Neither number ever grows, and the product a*n at least halves every pass
//! (the new a is at most half the old larger number, and the new n is the old
//! smaller one), so this ends after at most 511 passes; random input takes
//! about 180 (156 to 204 in a sample of 20,000). At the end n is gcd(a, p):
//! 1, unless a was a multiple of p,
//! and rule 4 gives the answer. Only the parity of the number of sign flips
//! is tracked.
//!
//! Every step is a comparison, a subtraction, a count of trailing zeros and a
//! shift. That makes the loop much cheaper than an exponentiation, and also
//! variable-time: its branches and pass count depend on the input. That is
//! fine for PDAs, whose inputs are public.
//!
//! The same step is written three times, for 256-, 128- and 64-bit numbers.
//! Both numbers only shrink, so once they fit in 128 (then 64) bits the loop
//! moves to cheaper arithmetic; on the machines measured, this halves the
//! cost compared with 256-bit arithmetic all the way.

/// p as (high 128 bits, low 128 bits). The pairs below are 256-bit numbers in
/// this order, so comparing pairs as tuples compares the numbers.
const P: (u128, u128) = ((1 << 127) - 1, u128::MAX - 18);

/// Whether the 256-bit little-endian number in `bytes` is 0 or a square mod
/// p, that is, whether its Jacobi symbol (a | p) is not -1. The number does
/// not need to be below p.
pub(crate) fn is_square(bytes: &[u8; 32]) -> bool {
    let mut lo = [0u8; 16];
    let mut hi = [0u8; 16];
    lo.copy_from_slice(&bytes[..16]);
    hi.copy_from_slice(&bytes[16..]);
    let mut a = (u128::from_le_bytes(hi), u128::from_le_bytes(lo));
    let mut n = P;
    if a == (0, 0) {
        return true;
    }

    // Bit 0 of `flips` is the parity of the sign flips; the other bits are
    // noise. The tests for a flip only need the low bits of the low word:
    // - n is 3 or 5 mod 8 exactly when bit 1 and bit 2 of n differ, so
    //   ((n >> 1) ^ (n >> 2)) & 1 is (2 | n) == -1, and k factors of 2 flip
    //   the sign when that and k are both odd: k & ((n >> 1) ^ (n >> 2));
    // - odd a and n are both 3 mod 4 exactly when bit 1 is set in both:
    //   ((a & n) >> 1) & 1.
    let mut flips: u32 = 0;

    // Make a odd before the first pass (n = p is odd).
    let k = trailing_zeros(a);
    if k > 0 {
        a = shift_right(a, k);
        flips ^= k & ((n.1 >> 1) ^ (n.1 >> 2)) as u32;
    }

    // 256 bits, while either number has a nonzero high half. a and n are odd
    // at the top of every pass.
    while (a.0 | n.0) != 0 {
        if a < n {
            flips ^= ((a.1 & n.1) >> 1) as u32;
            (a, n) = (subtract(n, a), a);
        } else {
            a = subtract(a, n);
        }
        if a == (0, 0) {
            return n != (0, 1) || flips & 1 == 0;
        }
        // a is even and nonzero, so 1 <= k <= 255.
        let k = trailing_zeros(a);
        a = shift_right(a, k);
        flips ^= k & ((n.1 >> 1) ^ (n.1 >> 2)) as u32;
    }

    // 128 bits: the same pass.
    let (mut a, mut n) = (a.1, n.1);
    while ((a | n) >> 64) != 0 {
        if a < n {
            flips ^= ((a & n) >> 1) as u32;
            (a, n) = (n - a, a);
        } else {
            a -= n;
        }
        if a == 0 {
            return n != 1 || flips & 1 == 0;
        }
        let k = a.trailing_zeros();
        a >>= k;
        flips ^= k & ((n >> 1) ^ (n >> 2)) as u32;
    }

    // 64 bits: the same pass, until a reaches 0.
    let (mut a, mut n) = (a as u64, n as u64);
    loop {
        if a < n {
            flips ^= ((a & n) >> 1) as u32;
            (a, n) = (n - a, a);
        } else {
            a -= n;
        }
        if a == 0 {
            // n is gcd(a, p): 1, unless the input was a multiple of p, whose
            // symbol is 0 (rule 4).
            return n != 1 || flips & 1 == 0;
        }
        let k = a.trailing_zeros();
        a >>= k;
        flips ^= k & ((n >> 1) ^ (n >> 2)) as u32;
    }
}

/// a - b for a >= b.
fn subtract(a: (u128, u128), b: (u128, u128)) -> (u128, u128) {
    let (lo, borrow) = a.1.overflowing_sub(b.1);
    (a.0 - b.0 - borrow as u128, lo)
}

/// The number of trailing zero bits of a, which is not 0.
fn trailing_zeros(a: (u128, u128)) -> u32 {
    if a.1 != 0 {
        a.1.trailing_zeros()
    } else {
        128 + a.0.trailing_zeros()
    }
}

/// a >> k for 1 <= k <= 255.
fn shift_right(a: (u128, u128), k: u32) -> (u128, u128) {
    if k < 128 {
        (a.0 >> k, (a.1 >> k) | (a.0 << (128 - k)))
    } else {
        (0, a.0 >> (k - 128))
    }
}

#[cfg(test)]
#[path = "tests/jacobi.rs"]
mod tests;
