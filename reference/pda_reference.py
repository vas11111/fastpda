#!/usr/bin/env python3
"""An independent reference for Solana PDA derivation, in pure Python.

It shares no code and no method with the Rust crate: SHA-256 comes from
hashlib, field arithmetic is Python's big integers, and the on-curve test
decompresses the point in full (RFC 8032 section 5.1.3, square root and all),
where the crate computes a Jacobi symbol. It follows curve25519-dalek's
decoding, which is what solana-pubkey runs: the sign bit never refuses an
encoding, and y >= p is read as y - p.

    python3 reference/pda_reference.py            # write reference/vectors.txt
    python3 reference/pda_reference.py --check    # re-derive every vector in it

The vectors are deterministic: the inputs come from SHA-256 in counter mode,
so the same script always writes the same file.
"""

import hashlib
import sys
from pathlib import Path

P = 2**255 - 19
D = (-121665 * pow(121666, P - 2, P)) % P
SQRT_M1 = pow(2, (P - 1) // 4, P)  # a square root of -1 (p = 5 mod 8)
MAX_SEEDS = 16
MAX_SEED_LEN = 32
PDA_MARKER = b"ProgramDerivedAddress"

VECTORS = Path(__file__).with_name("vectors.txt")

# Seed lists ["low-bump", n as 8 little-endian bytes] whose first off-curve
# bump is far below 255, for the program id sha256("fastpda low-bump
# search"). The counters came from a search in Rust (tests/solana.rs,
# search_low_bumps); the bumps written to the vectors are derived here.
LOW_BUMP_COUNTERS = [15725687, 41453911, 32788965, 9030244, 7301109, 7497205, 7319664, 1872682]


def decompress(encoding: bytes):
    """The point (x, y) that `encoding` decodes to, or None. Same answers as
    curve25519-dalek's CompressedEdwardsY::decompress."""
    assert len(encoding) == 32
    value = int.from_bytes(encoding, "little")
    sign = value >> 255
    # dalek reads y from the low 255 bits and does not refuse y >= p.
    y = (value & ((1 << 255) - 1)) % P
    u = (y * y - 1) % P
    v = (D * y * y + 1) % P
    # The candidate root of u/v (RFC 8032): x = u v^3 (u v^7)^((p-5)/8).
    x = (u * pow(v, 3, P) * pow(u * pow(v, 7, P), (P - 5) // 8, P)) % P
    vxx = (v * x * x) % P
    if vxx == u:
        pass
    elif vxx == (-u) % P:
        x = (x * SQRT_M1) % P
    else:
        return None
    # The sign bit picks the root. RFC 8032 refuses x = 0 with the sign bit
    # set; dalek does not (negating 0 gives 0), so neither does this.
    if (x & 1) != sign:
        x = (-x) % P
    # Check the point really is on -x^2 + y^2 = 1 + d x^2 y^2.
    assert (-x * x + y * y - 1 - D * x * x * y * y) % P == 0
    return (x, y)


def is_on_curve(encoding: bytes) -> bool:
    return decompress(encoding) is not None


def is_square(a: int) -> bool:
    """Euler's criterion: a is 0 or a square mod p."""
    return pow(a % P, (P - 1) // 2, P) != P - 1


def create_program_address(seeds, program_id: bytes):
    """The address, or "too-long" / "on-curve" for solana-pubkey's
    MaxSeedLengthExceeded / InvalidSeeds."""
    if len(seeds) > MAX_SEEDS or any(len(seed) > MAX_SEED_LEN for seed in seeds):
        return "too-long"
    digest = hashlib.sha256(b"".join(seeds) + program_id + PDA_MARKER).digest()
    if is_on_curve(digest):
        return "on-curve"
    return digest


def try_find_program_address(seeds, program_id: bytes):
    """(address, bump) for the first bump from 255 down to 1 that gives an
    address, or None; the bump needs one of the 16 seed slots."""
    if len(seeds) >= MAX_SEEDS or any(len(seed) > MAX_SEED_LEN for seed in seeds):
        return None
    for bump in range(255, 0, -1):
        address = create_program_address(seeds + [bytes([bump])], program_id)
        if isinstance(address, bytes):
            return (address, bump)
    return None


def self_test():
    # d is -121665/121666, -1 is a square, d is not.
    assert (D * 121666 + 121665) % P == 0
    assert SQRT_M1 * SQRT_M1 % P == P - 1
    assert is_square(P - 1) and not is_square(D) and not is_square(2)
    # The ed25519 base point: y = 4/5, and its known x.
    base_y = 4 * pow(5, P - 2, P) % P
    base_x = 15112221349535400772501151409588531511454012693041857206046113283949847762202
    assert decompress(base_y.to_bytes(32, "little")) == (base_x, base_y)
    # The identity (y = 1), its "negative zero" encoding, and y = 2.
    one = (1).to_bytes(32, "little")
    assert decompress(one) == (0, 1)
    assert decompress(bytes(one[:31]) + bytes([0x80])) == (0, 1)
    assert not is_on_curve((2).to_bytes(32, "little"))
    # A PDA everyone can check: the empty seed list's search ends somewhere.
    assert try_find_program_address([], bytes(32)) is not None


class Drbg:
    """SHA-256 in counter mode: deterministic inputs for the vectors."""

    def __init__(self, label: bytes):
        self.key = hashlib.sha256(label).digest()
        self.counter = 0
        self.buffer = b""

    def bytes(self, n: int) -> bytes:
        while len(self.buffer) < n:
            self.buffer += hashlib.sha256(self.key + self.counter.to_bytes(8, "little")).digest()
            self.counter += 1
        out, self.buffer = self.buffer[:n], self.buffer[n:]
        return out

    def below(self, n: int) -> int:
        return int.from_bytes(self.bytes(8), "little") % n

    def seeds(self, count: int, max_len: int):
        # Half the seeds are 32 bytes (a key), the rest 0..=max_len bytes.
        return [self.bytes(min(32, max_len) if self.below(2) == 0 else self.below(max_len + 1)) for _ in range(count)]


def encode_seeds(seeds) -> str:
    return f"{len(seeds)}:" + ",".join(seed.hex() for seed in seeds)


def decode_seeds(text: str):
    count, _, rest = text.partition(":")
    count = int(count)
    seeds = [bytes.fromhex(s) for s in rest.split(",")] if count else []
    assert len(seeds) == count, text
    return seeds


def curve_line(encoding: bytes) -> str:
    return f"curve {encoding.hex()} {int(is_on_curve(encoding))}"


def square_line(a: int) -> str:
    return f"square {a.to_bytes(32, 'little').hex()} {int(is_square(a))}"


def create_line(seeds, program_id: bytes) -> str:
    result = create_program_address(seeds, program_id)
    result = result.hex() if isinstance(result, bytes) else result
    return f"create {program_id.hex()} {encode_seeds(seeds)} {result}"


def find_line(seeds, program_id: bytes) -> str:
    found = try_find_program_address(seeds, program_id)
    result = f"{found[0].hex()} {found[1]}" if found else "none"
    return f"find {program_id.hex()} {encode_seeds(seeds)} {result}"


def generate():
    lines = [
        "# Known-answer vectors for fastpda, written by reference/pda_reference.py:",
        "# pure Python, hashlib SHA-256, big-integer field arithmetic, and an",
        "# on-curve test by full point decompression. Regenerate or re-check with",
        "#   python3 reference/pda_reference.py [--check]",
        "#",
        "#   curve  <32-byte hex> <1 if it decodes to a point, else 0>",
        "#   square <32-byte little-endian hex> <1 if 0 or a square mod 2^255-19, else 0>",
        "#   create <program id hex> <seeds> <address hex | on-curve | too-long>",
        "#   find   <program id hex> <seeds> <address hex> <bump> | none",
        "#",
        "# <seeds> is the seed count, a colon, and the seeds in hex joined by commas",
        "# (\"0:\" is no seeds, \"2:,ab\" is an empty seed and the byte 0xab).",
    ]
    rng = Drbg(b"fastpda known-answer vectors")

    # curve: edge encodings, then random strings.
    edges = []
    for y in range(64):
        edges.append(y)
        edges.append(P - 1 - y)
    for k in range(19):
        edges.append(P + k)  # the non-canonical encodings of 0 .. 18
    for bit in range(256):
        edges.append(1 << bit)
        edges.append((2**256 - 1) ^ (1 << bit))
    base_y = 4 * pow(5, P - 2, P) % P
    edges.append(base_y)
    for value in edges:
        for sign in (0, 1 << 255):
            lines.append(curve_line(((value % 2**255) | sign).to_bytes(32, "little")))
    for _ in range(1500):
        lines.append(curve_line(rng.bytes(32)))

    # square: small numbers, numbers around p and 2p, powers of two, random.
    values = list(range(128)) + [P - 1 - k for k in range(64)] + [P + k for k in range(64)]
    values += [2 * P + k for k in range(38)] + [2**256 - 1 - k for k in range(64)]
    values += [1 << j for j in range(256)] + [P + (1 << j) for j in range(255)]
    values += [int.from_bytes(rng.bytes(32), "little") for _ in range(1000)]
    for a in values:
        lines.append(square_line(a))

    # create: random seed lists around the limits, with and without a bump.
    for _ in range(300):
        count = rng.below(MAX_SEEDS + 2)
        seeds = rng.seeds(count, MAX_SEED_LEN + 1 if rng.below(8) == 0 else MAX_SEED_LEN)
        lines.append(create_line(seeds, rng.bytes(32)))
    for count in (0, 1, 15, 16, 17):
        for length in (0, 1, 32, 33):
            lines.append(create_line([bytes([count]) * length] * count, rng.bytes(32)))

    # find: random seed lists, common shapes, the limits, and low bumps.
    for _ in range(300):
        count = rng.below(MAX_SEEDS)
        lines.append(find_line(rng.seeds(count, MAX_SEED_LEN), rng.bytes(32)))
    for _ in range(50):
        program_id = rng.bytes(32)
        a, b, c = rng.bytes(32), rng.bytes(32), rng.bytes(32)
        lines.append(find_line([a, b, c], program_id))  # associated-token-account shape
        lines.append(find_line([b"bonding-curve", a], program_id))
        lines.append(find_line([b"config"], program_id))
        lines.append(find_line([b"metadata", program_id, a], program_id))
    for count in (0, 1, 15, 16, 17):
        for length in (0, 1, 32, 33):
            lines.append(find_line([bytes([length]) * length] * count, rng.bytes(32)))
    low_bump_program = hashlib.sha256(b"fastpda low-bump search").digest()
    for n in LOW_BUMP_COUNTERS:
        lines.append(find_line([b"low-bump", n.to_bytes(8, "little")], low_bump_program))

    VECTORS.write_text("\n".join(lines) + "\n")
    counts = {}
    for line in lines:
        if not line.startswith("#"):
            counts[line.split()[0]] = counts.get(line.split()[0], 0) + 1
    print(f"wrote {VECTORS}: {counts}")


def check():
    counts = {}
    for number, line in enumerate(VECTORS.read_text().splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        kind, *fields = line.split(" ")
        if kind == "curve":
            expected = curve_line(bytes.fromhex(fields[0]))
        elif kind == "square":
            expected = square_line(int.from_bytes(bytes.fromhex(fields[0]), "little"))
        elif kind == "create":
            expected = create_line(decode_seeds(fields[1]), bytes.fromhex(fields[0]))
        elif kind == "find":
            expected = find_line(decode_seeds(fields[1]), bytes.fromhex(fields[0]))
        else:
            raise SystemExit(f"line {number}: unknown kind {kind!r}")
        if line != expected:
            raise SystemExit(f"line {number} differs:\n  file:   {line}\n  script: {expected}")
        counts[kind] = counts.get(kind, 0) + 1
    print(f"{VECTORS}: all vectors re-derived: {counts}")


if __name__ == "__main__":
    self_test()
    if sys.argv[1:] == ["--check"]:
        check()
    elif sys.argv[1:] == []:
        generate()
    else:
        raise SystemExit(__doc__)
