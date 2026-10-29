# fastpda

Solana program-derived address (PDA) derivation, bit for bit the same as
[`solana-pubkey`](https://crates.io/crates/solana-pubkey), about three times
faster.

```rust
let (vault, bump) = fastpda::find_program_address(&[b"vault", user.as_ref()], &program_id);
```

`fastpda::find_program_address`, `try_find_program_address` and
`create_program_address` are drop-in replacements for the `Pubkey` methods of
the same names: same arguments, same results, same errors, same panic
messages. `fastpda::is_on_curve` is the on-curve test on its own.

## Why

A PDA is `sha256(seeds ‖ bump ‖ program_id ‖ "ProgramDerivedAddress")`, and it
is valid only if those 32 bytes are *not* the encoding of an ed25519 point (so
that no private key can exist for it). `find_program_address` tries bumps 255,
254, ... until the hash is off the curve. About half of all 32-byte strings
are points, so a search takes two tries on average: over a million random
searches, 50.0% needed one try, 25.0% two, 12.5% three, and the mean was
1.999.

Each try is one SHA-256 and one on-curve test, and the on-curve test is
nearly all of it. `solana-pubkey` answers it by asking curve25519-dalek to
decompress the point, which computes a square root: a field exponentiation of
about 254 squarings and a dozen multiplications. On an AMD Ryzen 9 9950X
(Zen 5) that takes about 2,400 ns, while a whole `create_program_address`
(hash and test) takes 2,460 to 2,660 ns depending on the seeds: the hash,
done with the CPU's SHA-256 instructions, is the small remainder.

fastpda answers the same yes/no question without computing the root, in about
820 ns, so a search for a typical PDA drops from about 4.9 µs to about 1.8 µs.

## How it works

1. Ed25519 points satisfy `-x² + y² = 1 + d·x²·y²` modulo `p = 2²⁵⁵ - 19`. The
   32 bytes hold `y` and the sign of `x`. Solving for `x²` gives
   `x² = u / v` with `u = y² - 1` and `v = d·y² + 1`, so the bytes encode a
   point exactly when `u / v` is a square mod p (0 included).
2. `v` is never 0 (that would need `y² = -1/d`, and `-1/d` is not a square
   mod p), and `u / v` is a square exactly when `u·v = (u / v)·v²` is. So the
   test is: is `u·v` 0 or a square? Computing `u·v` takes one squaring and two
   multiplications, done with [fiat-crypto](https://github.com/mit-plv/fiat-crypto)'s
   formally verified field arithmetic (the code curve25519-dalek's `fiat`
   backend uses).
3. Whether a number is a square mod a prime is its Legendre symbol, which
   equals the Jacobi symbol, and the Jacobi symbol can be computed like a
   binary gcd: subtract the smaller number from the larger, strip factors of
   two, swap when needed, and track the sign with three rules (`(a|n) =
   (a-n|n)`; a factor of 2 flips the sign when `n` is 3 or 5 mod 8; swapping
   flips it when both are 3 mod 4). That is about 180 passes of compares,
   subtractions and shifts, on 256-bit numbers, then 128-bit, then 64-bit as
   they shrink, instead of ~254 field squarings.
4. Encodings are read exactly as dalek reads them, including the ones RFC 8032
   rejects: the sign bit is ignored (including "negative zero", the sign bit
   set on `x = 0`), and the 19 non-canonical values `y = p .. 2²⁵⁵ - 1` are read
   as `y - p`.
5. The search hashes the seeds once and clones the SHA-256 state for each
   bump, instead of hashing everything again per try.

The source explains each step where it happens: `src/curve.rs` (the
reduction to a square test, the encodings) and `src/jacobi.rs` (the symbol).

## Correctness

A different answer would be a different address, so every test compares
against an implementation that shares no code with this one.

| what | compared with | cases in the default `cargo test` (about 5 s in debug) |
|---|---|---|
| Jacobi symbol | Euler's criterion `a^((p-1)/2)` via num-bigint | 3,000 small numbers; numbers at and around p, 2p and 2²⁵⁶; every power of two and p + 2ʲ (every shift length, across the 128-bit halves); 50,000 random 256-bit numbers; 2,000 squares and twice-squares (which are known non-squares); proptest: Euler, multiplicativity, `a + p`; 1,869 Python vectors |
| field arithmetic (`u·v`) | num-bigint | about 1,300 edge values (small y, p - k, the 19 non-canonical y, single bits, both signs); 100,000 random; proptest |
| on-curve test | curve25519-dalek's `decompress` (what solana-pubkey runs) | about 1,850 edge encodings (y = 0..255, p - k, all 19 non-canonical y against their canonical twins, the eight small-order points, every single bit set and cleared, repeated bytes, both signs); 2,000 real points (both signs, plus torsion); 200,000 random strings; 200,000 SHA-256 outputs; proptest; 2,820 Python vectors |
| `create` / `try_find` (byte API) | solana-pubkey | 20,000 random seed lists (0-15 seeds of 0-32 bytes, random program ids), each also checked with `create` at the found bump, at every bump above it, and at a random bump (about 60,000 creates); 12,000 common shapes; empty seeds; split versus joined seeds; every limit (15, 16, 17 seeds; 32- and 33-byte seeds; a long seed at each position) and the panic message; 8 inputs whose first valid bump is 229 to 235 (up to 26 rejected tries); proptest: 8 properties x 4,096 cases; 320 create and 528 find Python vectors |
| `Pubkey` API | solana-pubkey | 10,000 random seed lists, some over the limits; error conversions and messages; panic message; 5,000 `is_on_curve` against `Pubkey::is_on_curve` |
| mainnet | the chain | 18 well-known addresses: the Metaplex metadata accounts of USDC, USDT and wrapped SOL; the ProgramData accounts of Jupiter v6, Token Metadata, Raydium AMM v4 and Orca Whirlpools; the Raydium AMM v4 authority, Jupiter's event authority and the Jito stake pool withdraw authority; four Orca SOL/USDC whirlpools; four associated token accounts of those authorities. Each was checked on mainnet with `getAccountInfo` |

**The independent reference.** `reference/pda_reference.py` derives PDAs in
pure Python: SHA-256 from `hashlib`, field arithmetic on Python integers, and
an on-curve test that decompresses the point in full (RFC 8032's square root,
then a check of the curve equation) with dalek's decoding rules. It wrote
`reference/vectors.txt` (2,820 on-curve, 1,869 square, 320 create and 528 find
vectors, including the limits and the low bumps above, whose bumps it derived
on its own). `python3 reference/pda_reference.py --check` re-derives the
whole file, and CI runs it.

**Long sweeps**, ignored by default, run on every core with a printed seed
(`FASTPDA_SEED=<seed>` reruns the same inputs):

```sh
cargo test --release -- --ignored sweep --nocapture --test-threads=1
```

- 100,000,000 random 32-byte strings (with extra weight near p, past p, and
  on sparse values) against curve25519-dalek;
- 10,000,000 random seed lists against solana-pubkey, each checked as above,
  plus an over-the-limit list for one in eight;
- 10,000,000 random 256-bit numbers against Euler's criterion.

The run for this README found no difference: seeds `0xdb243051e252b505`
(on-curve), `0xefba4a9354e193d1` (10,000,000 searches and 31,248,795 creates)
and `0x4e676cabfa36429d` (Euler).

CI runs them every week.

**Fuzzing.** `fuzz/` has a cargo-fuzz target that reads arbitrary bytes as a
program id and a seed list and compares `is_on_curve`, `create` and
`try_find` with curve25519-dalek and solana-pubkey:

```sh
cargo +nightly fuzz run differential
```

A five-minute run on 8 processes (about 58 million inputs) found no
difference.

CI fuzzes for 30 minutes every week.

## Benchmarks

Per call, solana-pubkey against fastpda, on the same inputs:

<!-- bench:start -->
| CPU | on-curve test | create, ATA-like | find, ATA-like | find, pump-curve-like |
|---|---|---|---|---|
| AMD Ryzen 9 7950X (Zen 4) | 2,808 ns → **797 ns** (3.5x) | 2,910 ns → **907 ns** (3.2x) | 5,745 ns → **1,811 ns** (3.2x) | 5,758 ns → **1,802 ns** (3.2x) |
| AMD Ryzen 9 9950X (Zen 5) | 2,395 ns → **817 ns** (2.9x) | 2,492 ns → **924 ns** (2.7x) | 4,896 ns → **1,844 ns** (2.7x) | 4,906 ns → **1,832 ns** (2.7x) |

Each cell is solana-pubkey → fastpda per call, and the speedup.

<details><summary>AMD Ryzen 9 7950X (Zen 4): every benchmark (rustc 1.97.1 (8bab26f4f 2026-07-14), default flags)</summary>

| benchmark | case | solana-pubkey | fastpda | speedup |
|---|---|--:|--:|--:|
| on-curve test | - | 2,808 ns | 797 ns | 3.52x |
| create_program_address | 1x8B | 2,873 ns | 876 ns | 3.28x |
| create_program_address | 1x32B | 2,870 ns | 876 ns | 3.28x |
| create_program_address | pump-curve-like | 2,873 ns | 880 ns | 3.27x |
| create_program_address | ata-like | 2,910 ns | 907 ns | 3.21x |
| create_program_address | 4x8B | 2,875 ns | 886 ns | 3.24x |
| create_program_address | 4x32B | 2,900 ns | 906 ns | 3.20x |
| create_program_address | 8x32B | 2,957 ns | 962 ns | 3.08x |
| create_program_address | 15x8B | 2,920 ns | 926 ns | 3.15x |
| create_program_address | 15x32B | 3,079 ns | 1,083 ns | 2.84x |
| create_program_address | 16x8B | 2,921 ns | 926 ns | 3.15x |
| create_program_address | 16x32B | 3,066 ns | 1,077 ns | 2.85x |
| find_program_address | 1x8B | 5,535 ns | 1,723 ns | 3.21x |
| find_program_address | 1x32B | 5,866 ns | 1,844 ns | 3.18x |
| find_program_address | pump-curve-like | 5,758 ns | 1,802 ns | 3.20x |
| find_program_address | ata-like | 5,745 ns | 1,811 ns | 3.17x |
| find_program_address | 4x8B | 5,667 ns | 1,772 ns | 3.20x |
| find_program_address | 4x32B | 6,057 ns | 1,868 ns | 3.24x |
| find_program_address | 8x32B | 5,937 ns | 1,860 ns | 3.19x |
| find_program_address | 15x8B | 6,038 ns | 1,901 ns | 3.18x |
| find_program_address | 15x32B | 6,187 ns | 2,018 ns | 3.07x |
| find_program_address, ATA-like, by tries | 1 try | 2,917 ns | 667 ns | 4.37x |
| find_program_address, ATA-like, by tries | 2 tries | 5,848 ns | 1,538 ns | 3.80x |
| find_program_address, ATA-like, by tries | 3 tries | 8,780 ns | 2,640 ns | 3.33x |
| find_program_address, ATA-like, by tries | 4 tries | 11,715 ns | 3,586 ns | 3.27x |
| find_program_address, ATA-like, by tries | 5 tries | 14,640 ns | 4,510 ns | 3.25x |
| find_program_address, ATA-like, by tries | 6 tries | 17,586 ns | 5,419 ns | 3.25x |
| find_program_address, ATA-like, by tries | 7 tries | 20,508 ns | 6,324 ns | 3.24x |
| find_program_address, ATA-like, by tries | 8 tries | 23,445 ns | 7,229 ns | 3.24x |

</details>

<details><summary>AMD Ryzen 9 9950X (Zen 5): every benchmark (rustc 1.92.0 (ded5c06cf 2025-12-08), default flags)</summary>

| benchmark | case | solana-pubkey | fastpda | speedup |
|---|---|--:|--:|--:|
| on-curve test | - | 2,395 ns | 817 ns | 2.93x |
| create_program_address | 1x8B | 2,479 ns | 894 ns | 2.77x |
| create_program_address | 1x32B | 2,465 ns | 894 ns | 2.76x |
| create_program_address | pump-curve-like | 2,461 ns | 898 ns | 2.74x |
| create_program_address | ata-like | 2,492 ns | 924 ns | 2.70x |
| create_program_address | 4x8B | 2,466 ns | 902 ns | 2.74x |
| create_program_address | 4x32B | 2,490 ns | 923 ns | 2.70x |
| create_program_address | 8x32B | 2,545 ns | 974 ns | 2.61x |
| create_program_address | 15x8B | 2,508 ns | 949 ns | 2.64x |
| create_program_address | 15x32B | 2,658 ns | 1,097 ns | 2.42x |
| create_program_address | 16x8B | 2,513 ns | 951 ns | 2.64x |
| create_program_address | 16x32B | 2,654 ns | 1,091 ns | 2.43x |
| find_program_address | 1x8B | 4,727 ns | 1,765 ns | 2.68x |
| find_program_address | 1x32B | 4,999 ns | 1,864 ns | 2.68x |
| find_program_address | pump-curve-like | 4,906 ns | 1,832 ns | 2.68x |
| find_program_address | ata-like | 4,896 ns | 1,844 ns | 2.66x |
| find_program_address | 4x8B | 4,823 ns | 1,813 ns | 2.66x |
| find_program_address | 4x32B | 5,193 ns | 1,912 ns | 2.72x |
| find_program_address | 8x32B | 5,077 ns | 1,894 ns | 2.68x |
| find_program_address | 15x8B | 5,156 ns | 1,940 ns | 2.66x |
| find_program_address | 15x32B | 5,335 ns | 2,051 ns | 2.60x |
| find_program_address, ATA-like, by tries | 1 try | 2,485 ns | 645 ns | 3.85x |
| find_program_address, ATA-like, by tries | 2 tries | 4,998 ns | 1,669 ns | 3.00x |
| find_program_address, ATA-like, by tries | 3 tries | 7,492 ns | 2,633 ns | 2.85x |
| find_program_address, ATA-like, by tries | 4 tries | 10,006 ns | 3,602 ns | 2.78x |
| find_program_address, ATA-like, by tries | 5 tries | 12,485 ns | 4,534 ns | 2.75x |
| find_program_address, ATA-like, by tries | 6 tries | 15,053 ns | 5,477 ns | 2.75x |
| find_program_address, ATA-like, by tries | 7 tries | 17,512 ns | 6,445 ns | 2.72x |
| find_program_address, ATA-like, by tries | 8 tries | 19,991 ns | 7,374 ns | 2.71x |

</details>
<!-- bench:end -->

The two AMD machines were measured on servers that were running other work:
the benchmarks ran at the lowest scheduling priority (`nice -n 19`) pinned to
8 of their 32 hardware threads, so their absolute numbers carry some noise,
while every pair of figures shares the same conditions. The Zen 5 run used
rustc 1.92.0, the Zen 4 run rustc 1.97.1.

How they are measured: [criterion](https://github.com/criterion-rs/criterion.rs),
50 samples of 3 seconds per benchmark, release build with fat LTO and the
default target CPU. Each benchmark cycles through 1,024 distinct random inputs,
so neither side gets a branch predictor or cache trained on one input. Both
sides hash with hardware SHA-256 (see the note under Usage), so the
difference is the on-curve test and the hashing layout. `find_by_tries` times
the search on inputs whose first valid bump is found at try 1 to 8, so the
cost per try shows. Shapes: `NxLB` is N seeds of L bytes; `ata-like` is an
associated token account (`[owner, token program, mint]`); `pump-curve-like`
is a short string and a key (`["bonding-curve", mint]`).

To run them and add a machine:

```sh
bench/run.sh                # about 5 minutes; writes bench/results/<cpu-model>.csv
python3 bench/render.py     # rewrites the tables above from every CSV
```

`bench/run.sh` reads the CPU model from `sysctl` (macOS) or `lscpu` (Linux)
and records the rustc version and `RUSTFLAGS`. It needs Rust (stable),
Python 3 and, on aarch64, a C compiler. No cargo features are needed: on
x86-64 both SHA-256 implementations pick SHA-NI at runtime. Run it on an idle
machine.

## Usage

```toml
[dependencies]
fastpda = { git = "https://github.com/vas11111/fastpda" }
```

```rust
use solana_pubkey::Pubkey;

fn vault(user: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    fastpda::find_program_address(&[b"vault", user.as_ref()], program_id)
}

fn check(user: &Pubkey, bump: u8, program_id: &Pubkey) -> bool {
    fastpda::create_program_address(&[b"vault", user.as_ref(), &[bump]], program_id).is_ok()
}
```

| item | |
|---|---|
| `find_program_address(seeds, &Pubkey) -> (Pubkey, u8)` | panics where solana-pubkey's does, with the same message |
| `try_find_program_address(seeds, &Pubkey) -> Option<(Pubkey, u8)>` | `None` for 16 or more seeds or a seed over 32 bytes |
| `create_program_address(seeds, &Pubkey) -> Result<Pubkey, PubkeyError>` | `MaxSeedLengthExceeded` / `InvalidSeeds` exactly when solana-pubkey returns them |
| `is_on_curve(&[u8; 32]) -> bool` | curve25519-dalek's `decompress().is_some()` |
| `bytes::{find, try_find, create}_program_address` | the same over `[u8; 32]`, with `fastpda::Error` |

**Features.** The default `solana-pubkey` feature provides the `Pubkey`
functions, for solana-pubkey 4.x. With `default-features = false` the crate
depends only on `sha2` and `fiat-crypto` and offers `bytes::*` and
`is_on_curve`; use that with any other `Pubkey` type
(`Pubkey::new_from_array` / `to_bytes`). The crate is `no_std` and has no
`unsafe` code (`#![forbid(unsafe_code)]`).

**SHA-256 on aarch64.** fastpda hashes with `sha2` 0.11, which uses the
ARMv8 SHA-256 instructions when the CPU has them. solana-pubkey uses `sha2`
0.10, which does so only with its `asm` feature; fastpda's benches and tests
turn that on, so both sides hash in hardware there. In your own build without
it, solana-pubkey's hashing is slower on aarch64 (Apple silicon, Graviton)
and the gap to fastpda wider than the tables show. On x86-64 both versions
pick SHA-NI at runtime.

## Limits

- **Not constant-time.** The Jacobi loop branches on its input and its running
  time depends on it. That is fine here: a PDA's seeds and program id are
  public, and so is the address, so there is nothing for timing to reveal.
  Do not use `is_on_curve` on secret data.
- **Off-chain only.** Inside a Solana program, solana-pubkey's functions call
  the runtime's syscalls, which is far cheaper in compute units than doing the
  math in the program.
- **The speedup is per derivation.** If deriving addresses is not where your
  time goes, this changes little.

## MSRV

Rust 1.85. CI builds the library with 1.85, with dependencies resolved the way
Cargo 1.84+ resolves them for that toolchain (the newest versions that support
it); the newest solana-pubkey 4.x releases themselves need 1.89.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Unless you explicitly state
otherwise, any contribution intentionally submitted for inclusion in this
crate, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions.
