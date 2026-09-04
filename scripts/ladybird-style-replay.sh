#!/usr/bin/env bash
# Replay StyleEngine boundary captures engine-only (no browser, no layout).
#
#   scripts/ladybird-style-replay.sh [--suite NAME] capture.sg ...
#
# Binary: ladybird/Build/distribution/bin/style-replay (build:
#   cd ladybird/Build/distribution && ninja bin/style-replay)
#
# At ladybird a1db2e3a a full StyleBench capture fails replay; use
# scripts/browser-bench.sh ladybird --internals for the working style clock.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPLAY="$ROOT/ladybird/Build/distribution/bin/style-replay"
[ -x "$REPLAY" ] || { echo "style-replay not found: $REPLAY"; exit 1; }
exec "$REPLAY" "$@"
