#!/usr/bin/env bash
# 20k race for one generated suite: sibling (` ` `>` `+` `~`),
# structural (adds :first-child / :last-child / :first-of-type /
# :last-of-type / :only-of-type / :empty), nth (:nth-child(2n+1) /
# :nth-last-child(3n) / :nth-of-type(3n) / :nth-last-of-type(4n)),
# ba (::before / ::after subjects) or media (5k elements, @media blocks,
# 55 viewport resizes instead of DOM edits). Not on make bench-style.
#
#   ./scripts/bench-suite.sh sibling
#   ./scripts/bench-suite.sh structural
#   ./scripts/bench-suite.sh nth
#   ./scripts/bench-suite.sh ba
#   ./scripts/bench-suite.sh media
set -euo pipefail

SUITE="${1:-}"
case "$SUITE" in
    sibling|structural|nth|ba|media) ;;
    *) echo "usage: $0 sibling|structural|nth|ba|media" >&2; exit 2 ;;
esac

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURES="$ROOT/fixtures"
RECEIPTS="$ROOT/receipts"
FIX="$FIXTURES/$SUITE.stylebench"
OUT="$SUITE"20k
GEN="--$SUITE"
case "$SUITE" in
    ba) GEN="--before-after" ;;
    media) OUT=media5k ;;
esac
CARGO="${CARGO:-cargo}"
CCC="${CCC:-ccc}"

if [ ! -f "$ROOT/stylo/Cargo.toml" ]; then
    echo "stylo/ missing — run: git submodule update --init --depth 1" >&2
    exit 1
fi

# Full dumps (70-200 MB at 189 longhands) land in receipts/full/, gitignored;
# the committed receipt is the header plus a digest of the body.
FULL="$RECEIPTS/full"
mkdir -p "$FIXTURES" "$FULL"
echo "== generate $SUITE 20k =="
"$CARGO" run -q -p stylebench-gen --release -- "$GEN" > "$FIX"

echo "== stylo release =="
"$CARGO" run -q --release --manifest-path "$ROOT/stylo-runner/Cargo.toml" -- \
    "$FIX" > "$FULL/$OUT.stylo.txt"

echo "== cc -O (warm) =="
"$CCC" build run -O "$ROOT/engine/stylebench_cc.ccs" -- "$FIX" > /dev/null
echo "== cc -O =="
"$CCC" build run -O "$ROOT/engine/stylebench_cc.ccs" -- "$FIX" \
    > "$FULL/$OUT.cc.txt"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
grep -v '^#' "$FULL/$OUT.stylo.txt" > "$tmp/stylo"
grep -v '^#' "$FULL/$OUT.cc.txt" > "$tmp/cc"
cmp "$tmp/stylo" "$tmp/cc"
echo "OK $OUT"
"$ROOT/scripts/receipt.sh" "$FULL/$OUT.stylo.txt" "$RECEIPTS/$OUT.stylo.txt"
"$ROOT/scripts/receipt.sh" "$FULL/$OUT.cc.txt" "$RECEIPTS/$OUT.cc.txt"
grep '^# TIME' "$RECEIPTS/$OUT.stylo.txt" "$RECEIPTS/$OUT.cc.txt"
