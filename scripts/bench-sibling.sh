#!/usr/bin/env bash
# 20k sibling combinator race. Not on make bench-style.
#
#   ./scripts/bench-sibling.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURES="$ROOT/fixtures"
RECEIPTS="$ROOT/receipts"
FIX="$FIXTURES/sibling.stylebench"
CARGO="${CARGO:-cargo}"
CCC="${CCC:-ccc}"

if [ ! -f "$ROOT/stylo/Cargo.toml" ]; then
    echo "stylo/ missing — run: git submodule update --init --depth 1" >&2
    exit 1
fi

mkdir -p "$FIXTURES" "$RECEIPTS"
echo "== generate sibling 20k =="
"$CARGO" run -q -p stylebench-gen --release -- --sibling > "$FIX"

echo "== stylo release =="
"$CARGO" run -q --release --manifest-path "$ROOT/stylo-runner/Cargo.toml" -- \
    "$FIX" > "$RECEIPTS/sibling20k.stylo.txt"

echo "== cc -O (warm) =="
"$CCC" build run -O "$ROOT/engine/stylebench_cc.ccs" -- "$FIX" > /dev/null
echo "== cc -O =="
"$CCC" build run -O "$ROOT/engine/stylebench_cc.ccs" -- "$FIX" \
    > "$RECEIPTS/sibling20k.cc.txt"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
grep -v '^#' "$RECEIPTS/sibling20k.stylo.txt" > "$tmp/stylo"
grep -v '^#' "$RECEIPTS/sibling20k.cc.txt" > "$tmp/cc"
cmp "$tmp/stylo" "$tmp/cc"
echo OK sibling20k
grep '^# TIME' "$RECEIPTS/sibling20k.stylo.txt" "$RECEIPTS/sibling20k.cc.txt"
