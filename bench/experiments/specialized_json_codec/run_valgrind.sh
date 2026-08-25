#!/usr/bin/env bash
# Deterministic instruction counts for the specialized codec (experiment).
#
# Wall-clock numbers on a shared machine move around; callgrind's Ir count does
# not. Each case is run twice with a different iteration count and the two are
# subtracted, so interpreter startup and warmup drop out and what remains is the
# cost of one call.
#
# Both counts are taken well past the ramp: CPython's adaptive interpreter and
# pymalloc keep shaving a little off the per-call cost for the first few hundred
# iterations, so a 100-vs-400 subtraction overstates it by ~5%. 1000 vs 3000 is
# flat to well under a percent.
#
#   bench/experiments/specialized_json_codec/run_valgrind.sh [outdir]
set -euo pipefail

cd "$(dirname "$0")/../../.."
PY=${PY:-.venv312/bin/python}
# Per-invocation directory: two concurrent runs sharing profile filenames would
# subtract each other's counts and produce nonsense.
OUT=${1:-target/callgrind/run-$$}
LOW=${LOW:-1000}
HIGH=${HIGH:-3000}
mkdir -p "$OUT"

CASES=(
  "load:serpyco-rs JSON codec (baseline)"
  "load:specialized codec"
  "load:specialized codec, ordered (ORACLE)"
  "load:specialized scanner only (no objects)"
  "load:msgspec -> msgspec.Struct"
  "dump:serpyco-rs JSON codec (baseline)"
  "dump:specialized codec"
  "dump:msgspec <- msgspec.Struct"
)

run() { # case iterations tag
  SKIP_AFFINITY=1 valgrind --tool=callgrind --callgrind-out-file="$OUT/$3.out" \
    --quiet "$PY" -m bench.experiments.specialized_json_codec.run_bench \
    --callgrind "$1" --iterations "$2" 2>/dev/null
  grep -m1 '^summary:' "$OUT/$3.out" | awk '{print $2}'
}

printf '%-42s %14s\n' 'case' 'Ir / call'
printf '%-42s %14s\n' '------------------------------------------' '--------------'
i=0
for case in "${CASES[@]}"; do
  i=$((i + 1))
  lo=$(run "$case" "$LOW" "case${i}_lo")
  hi=$(run "$case" "$HIGH" "case${i}_hi")
  printf '%-42s %14d\n' "$case" $(((hi - lo) / (HIGH - LOW)))
done

echo
echo "profiles in $OUT (callgrind_annotate $OUT/case2_hi.out)"
