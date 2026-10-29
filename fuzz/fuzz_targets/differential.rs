//! Differential fuzzing: fastpda against solana-pubkey and curve25519-dalek.
//!
//!     cargo +nightly fuzz run differential
//!
//! The input is read as a program id (its first 32 bytes, zero-padded), then
//! a seed list: a byte giving a seed's length (mod 40, so seeds of 33..39
//! bytes break the limit) followed by that many bytes, repeated, up to 20
//! seeds. Each input checks:
//! - `is_on_curve` on the program id bytes against dalek's decompression;
//! - `create_program_address` on the seed list against solana-pubkey's;
//! - `try_find_program_address` on the seed list against solana-pubkey's,
//!   and the found address against `create` with its bump.

#![no_main]

use curve25519_dalek::edwards::CompressedEdwardsY;
use libfuzzer_sys::fuzz_target;
use solana_pubkey::Pubkey;

fuzz_target!(|data: &[u8]| {
    let mut program_id = [0u8; 32];
    let head = data.len().min(32);
    program_id[..head].copy_from_slice(&data[..head]);
    let mut rest = &data[head..];

    assert_eq!(
        fastpda::is_on_curve(&program_id),
        CompressedEdwardsY(program_id).decompress().is_some(),
        "on-curve test, bytes {program_id:02x?}"
    );

    let mut seeds: Vec<&[u8]> = Vec::new();
    while let Some((&len, tail)) = rest.split_first() {
        if seeds.len() == 20 {
            break;
        }
        let len = (len as usize % 40).min(tail.len());
        seeds.push(&tail[..len]);
        rest = &tail[len..];
    }

    let program = Pubkey::new_from_array(program_id);
    assert_eq!(
        fastpda::create_program_address(&seeds, &program),
        Pubkey::create_program_address(&seeds, &program),
        "create, seeds {seeds:02x?}"
    );

    let found = fastpda::try_find_program_address(&seeds, &program);
    assert_eq!(
        found,
        Pubkey::try_find_program_address(&seeds, &program),
        "find, seeds {seeds:02x?}"
    );
    if let Some((address, bump)) = found {
        let bump = [bump];
        seeds.push(&bump);
        assert_eq!(
            fastpda::create_program_address(&seeds, &program),
            Ok(address)
        );
    }
});
