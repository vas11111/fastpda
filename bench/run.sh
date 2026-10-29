#!/usr/bin/env bash
# Runs the criterion benchmarks and writes bench/results/<cpu-model>.csv.
#
#   bench/run.sh              all benchmarks (about 5 minutes)
#   bench/run.sh find_by      extra arguments go to criterion (here, a filter)
#
# Run it on an idle machine, plugged in. The compiler flags are cargo's
# defaults unless RUSTFLAGS is set; the CSV records them. Afterwards,
# `python3 bench/render.py` updates the README tables from all the CSVs.
set -euo pipefail
cd "$(dirname "$0")/.."

case "$(uname -s)" in
  Darwin) cpu=$(sysctl -n machdep.cpu.brand_string) ;;
  Linux) cpu=$(lscpu | sed -n 's/^Model name:[[:space:]]*//p' | head -n 1) ;;
  *) echo "bench/run.sh: unsupported OS $(uname -s)" >&2; exit 1 ;;
esac
[ -n "$cpu" ] || { echo "bench/run.sh: could not read the CPU model" >&2; exit 1; }
# "AMD Ryzen 9 9950X 16-Core Processor" -> amd-ryzen-9-9950x
slug=$(printf '%s' "$cpu" | sed -E 's/ [0-9]+-Core Processor$//; s/\((R|TM)\)//g' \
  | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//')

target_dir=$(cargo metadata --format-version 1 --no-deps \
  | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
# Start clean, so the CSV holds this run only.
rm -rf "$target_dir/criterion"

cargo bench --bench pda -- "$@"

out="bench/results/$slug.csv"
python3 - "$target_dir/criterion" "$cpu" "$(rustc --version)" "${RUSTFLAGS:-}" > "$out" <<'PY'
import csv, json, sys
from pathlib import Path

root, cpu, rustc, rustflags = sys.argv[1:5]
groups = ["on_curve", "create_program_address", "find_program_address", "find_by_tries"]
cases = ["", "1x8B", "1x32B", "pump-curve-like", "ata-like", "4x8B", "4x32B", "8x32B", "15x8B", "15x32B", "16x8B", "16x32B"]
rows = []
for meta in Path(root).glob("**/new/benchmark.json"):
    bench = json.loads(meta.read_text())
    estimates = json.loads(meta.with_name("estimates.json").read_text())
    # Criterion's headline figure: the slope of time over iterations.
    estimate = estimates.get("slope") or estimates["mean"]
    rows.append([
        cpu, rustc, rustflags,
        bench["group_id"], bench["value_str"] or "", bench["function_id"],
        f"{estimate['point_estimate']:.1f}",
        f"{estimate['confidence_interval']['lower_bound']:.1f}",
        f"{estimate['confidence_interval']['upper_bound']:.1f}",
    ])

def key(row):
    # The order the bench declares them in; solana-pubkey before fastpda.
    group, case, impl = row[3], row[4], row[5]
    position = int(case) if case.isdigit() else cases.index(case) if case in cases else len(cases)
    return (groups.index(group) if group in groups else len(groups), position, case, impl != "solana-pubkey")

writer = csv.writer(sys.stdout, lineterminator="\n")
writer.writerow(["cpu", "rustc", "rustflags", "group", "case", "impl", "ns", "ns_low", "ns_high"])
writer.writerows(sorted(rows, key=key))
PY
echo "wrote $out ($(($(wc -l < "$out") - 1)) benchmarks)"
