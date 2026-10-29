#!/usr/bin/env python3
"""Renders the README benchmark tables from bench/results/*.csv.

    python3 bench/render.py

Replaces everything between the bench markers in README.md. CPUs listed in
PENDING but without a CSV get a "pending" row.
"""

import csv
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
README = ROOT / "README.md"
START, END = "<!-- bench:start -->", "<!-- bench:end -->"

# Microarchitectures for the model names lscpu prints.
ARCH = {"9950X": "Zen 5", "7950X": "Zen 4"}
# Machines the README promises numbers for.
PENDING = ["AMD Ryzen 9 9950X (Zen 5)", "AMD Ryzen 9 7950X (Zen 4)"]

GROUPS = {
    "on_curve": "on-curve test",
    "create_program_address": "create_program_address",
    "find_program_address": "find_program_address",
    "find_by_tries": "find_program_address, ATA-like, by tries",
}
SUMMARY = [
    ("on-curve test", "on_curve", ""),
    ("create, ATA-like", "create_program_address", "ata-like"),
    ("find, ATA-like", "find_program_address", "ata-like"),
    ("find, pump-curve-like", "find_program_address", "pump-curve-like"),
]


def display_name(cpu: str) -> str:
    name = re.sub(r" \d+-Core Processor$", "", cpu.strip())
    for model, arch in ARCH.items():
        if model in name:
            name += f" ({arch})"
    return name


def ns(value: float) -> str:
    return f"{value:,.0f} ns" if value >= 100 else f"{value:,.1f} ns"


def load():
    machines = {}
    for path in sorted((ROOT / "bench" / "results").glob("*.csv")):
        rows = list(csv.DictReader(path.open()))
        if not rows:
            continue
        times = {(r["group"], r["case"], r["impl"]): float(r["ns"]) for r in rows}
        machines[display_name(rows[0]["cpu"])] = {
            "rustc": rows[0]["rustc"],
            "rustflags": rows[0]["rustflags"],
            "times": times,
            "order": list(dict.fromkeys((r["group"], r["case"]) for r in rows)),
        }
    return machines


def cell(times, group, case):
    theirs, ours = times.get((group, case, "solana-pubkey")), times.get((group, case, "fastpda"))
    if theirs is None or ours is None:
        return "-"
    return f"{ns(theirs)} → **{ns(ours)}** ({theirs / ours:.1f}x)"


def render(machines) -> str:
    out = ["| CPU | " + " | ".join(name for name, _, _ in SUMMARY) + " |"]
    out.append("|---" * (len(SUMMARY) + 1) + "|")
    names = list(machines) + [name for name in PENDING if name not in machines]
    for name in names:
        if name in machines:
            cells = [cell(machines[name]["times"], g, c) for _, g, c in SUMMARY]
        else:
            cells = ["pending"] * len(SUMMARY)
        out.append(f"| {name} | " + " | ".join(cells) + " |")
    out.append("")
    out.append("Each cell is solana-pubkey → fastpda per call, and the speedup.")
    for name, machine in machines.items():
        flags = f", `RUSTFLAGS={machine['rustflags']}`" if machine["rustflags"] else ", default flags"
        out.append("")
        out.append(f"<details><summary>{name}: every benchmark ({machine['rustc']}{flags})</summary>")
        out.append("")
        out.append("| benchmark | case | solana-pubkey | fastpda | speedup |")
        out.append("|---|---|--:|--:|--:|")
        for group, case in machine["order"]:
            times = machine["times"]
            theirs, ours = times.get((group, case, "solana-pubkey")), times.get((group, case, "fastpda"))
            if theirs is None or ours is None:
                continue
            label = (f"{case} try" if case == "1" else f"{case} tries") if group == "find_by_tries" else case or "-"
            out.append(f"| {GROUPS.get(group, group)} | {label} | {ns(theirs)} | {ns(ours)} | {theirs / ours:.2f}x |")
        out.append("")
        out.append("</details>")
    return "\n".join(out)


def main():
    text = README.read_text()
    before, rest = text.split(START, 1)
    _, after = rest.split(END, 1)
    README.write_text(f"{before}{START}\n{render(load())}\n{END}{after}")
    print(f"updated {README.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
