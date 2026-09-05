#!/usr/bin/env bash
# Build libstylecc and run the ABI smoke test.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CCC="${CCC:-ccc}"
"$ROOT/scripts/build-libstylecc.sh"
mkdir -p "$ROOT/bin"
CFLAGS=$("$CCC" --print-cflags)
cc -O2 -I"$ROOT/engine" $CFLAGS \
  "$ROOT/engine/test_stylecc_abi.c" \
  "$ROOT/lib/libstylecc.a" \
  -lpthread -lm \
  -o "$ROOT/bin/test_stylecc_abi"
exec "$ROOT/bin/test_stylecc_abi"
