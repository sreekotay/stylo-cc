#!/usr/bin/env bash
# 20k race for one generated suite: sibling (` ` `>` `+` `~`),
# structural (adds :first-child / :last-child / :first-of-type /
# :last-of-type / :only-of-type / :empty) or nth (:nth-child(2n+1) /
# :nth-last-child(3n) / :nth-of-type(3n) / :nth-last-of-type(4n)).
# Not on make bench-style.
#
#   ./scripts/bench-suite.sh sibling
#   ./scripts/bench-suite.sh structural
#   ./scripts/bench-suite.sh nth
set -euo pipefail

SUITE="${1:-}"
case "$SUITE" in
    sibling|structural|nth) ;;
    *) echo "usage: $0 sibling|structural|nth" >&2; exit 2 ;;
esac

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURES="$ROOT/fixtures"
RECEIPTS="$ROOT/receipts"
FIX="$FIXTURES/$SUITE.stylebench"
OUT="$SUITE"20k
CARGO="${CARGO:-cargo}"
CCC="${CCC:-ccc}"

if [ ! -f "$ROOT/stylo/Cargo.toml" ]; then
    echo "stylo/ missing — run: git submodule update --init --depth 1" >&2
    exit 1
fi

mkdir -p "$FIXTURES" "$RECEIPTS"
echo "== generate $SUITE 20k =="
"$CARGO" run -q -p stylebench-gen --release -- "--$SUITE" > "$FIX"

echo "== stylo release =="
"$CARGO" run -q --release --manifest-path "$ROOT/stylo-runner/Cargo.toml" -- \
    "$FIX" > "$RECEIPTS/$OUT.stylo.txt"

echo "== cc -O (warm) =="
"$CCC" build run -O "$ROOT/engine/stylebench_cc.ccs" -- "$FIX" > /dev/null
echo "== cc -O =="
"$CCC" build run -O "$ROOT/engine/stylebench_cc.ccs" -- "$FIX" \
    > "$RECEIPTS/$OUT.cc.txt"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
grep -v '^#' "$RECEIPTS/$OUT.stylo.txt" > "$tmp/stylo"
grep -v '^#' "$RECEIPTS/$OUT.cc.txt" > "$tmp/cc"
cmp "$tmp/stylo" "$tmp/cc"
echo "OK $OUT"
grep '^# TIME' "$RECEIPTS/$OUT.stylo.txt" "$RECEIPTS/$OUT.cc.txt"
