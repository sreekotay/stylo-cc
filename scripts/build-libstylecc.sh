#!/usr/bin/env bash
# Build lib/libstylecc.a (Concurrent-C StyleBench engine + C ABI) for Ladybird.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CCC="${CCC:-ccc}"
mkdir -p "$ROOT/out" "$ROOT/lib"
"$CCC" --compile -O "$ROOT/engine/stylebench_cc.ccs" -o "$ROOT/out/stylecc.o"
RUNTIME=$("$CCC" --print-libs | tr ' ' '\n' | grep 'concurrent_c\.c$' | head -1)
CFLAGS=$("$CCC" --print-cflags)
cc -c -O2 -DCC_ENABLE_ASYNC $CFLAGS "$RUNTIME" -o "$ROOT/out/concurrent_c.o"
rm -f "$ROOT/lib/libstylecc.a"
ar rcs "$ROOT/lib/libstylecc.a" "$ROOT/out/stylecc.o" "$ROOT/out/concurrent_c.o"
echo "built $ROOT/lib/libstylecc.a"
nm "$ROOT/lib/libstylecc.a" | grep 'stylecc_create' | head -3
