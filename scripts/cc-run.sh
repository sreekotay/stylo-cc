#!/usr/bin/env bash
# Build and run the CC StyleBench runner (engine + thin main).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CCC="${CCC:-ccc}"
OPT="${CC_OPT:---}" # pass -O for release via CC_OPT=-O
mkdir -p "$ROOT/out" "$ROOT/bin"
if [ "$OPT" = "-O" ]; then
  "$CCC" --compile -O "$ROOT/engine/stylebench_cc.ccs" -o "$ROOT/out/stylecc.o"
else
  "$CCC" --compile "$ROOT/engine/stylebench_cc.ccs" -o "$ROOT/out/stylecc.o"
fi
RUNTIME=$("$CCC" --print-libs | tr ' ' '\n' | grep 'concurrent_c\.c$' | head -1)
CFLAGS=$("$CCC" --print-cflags)
LIBS=$("$CCC" --print-libs)
cc -c -O2 -DCC_ENABLE_ASYNC $CFLAGS "$RUNTIME" -o "$ROOT/out/concurrent_c.o"
cc -c $CFLAGS "$ROOT/engine/stylebench_main.c" -o "$ROOT/out/stylebench_main.o"
# print-libs includes the .c file; link object + pthread/m instead
cc -o "$ROOT/bin/stylebench_cc" "$ROOT/out/stylebench_main.o" "$ROOT/out/stylecc.o" "$ROOT/out/concurrent_c.o" -lpthread -lm
exec "$ROOT/bin/stylebench_cc" "$@"
