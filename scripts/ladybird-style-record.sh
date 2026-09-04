#!/usr/bin/env bash
# Record a StyleEngine boundary trace while Ladybird runs StyleBench.
#
#   scripts/ladybird-style-record.sh OUTDIR [suite-name]
#
# Requires: ladybird/ built (Distribution), WebDriver in
#   ladybird/Build/distribution/bin/Ladybird.app/Contents/MacOS/WebDriver,
#   StyleBench served locally (scripts/browser-bench.sh serve --conservative).
#
# Output: OUTDIR/stylebench-*.sg (one per WebContent process). Replay with
#   scripts/ladybird-style-replay.sh OUTDIR/stylebench-*.sg
#
# At ladybird a1db2e3a full StyleBench replay still fails upstream; this records
# correctly but style-replay hits missing deferred-geometry arms + a segfault.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LB="$ROOT/ladybird"
WD="$LB/Build/distribution/bin/Ladybird.app/Contents/MacOS/WebDriver"
PORT="${STYLEBENCH_PORT:-8765}"
URL="http://127.0.0.1:${PORT}/StyleBench/index.html?iterationCount=1"
OUT="${1:?usage: $0 OUTDIR [suite-name]}"
SUITE="${2:-}"
[ -x "$WD" ] || { echo "WebDriver not found: $WD (build: scripts/browser-bench.sh ladybird-build; then ninja bin/style-replay in ladybird/Build/distribution)"; exit 1; }
curl -sf "http://127.0.0.1:${PORT}/StyleBench/index.html" >/dev/null || {
  echo "StyleBench not served at :${PORT}; in another terminal: scripts/browser-bench.sh serve --conservative --port ${PORT}"
  exit 1
}
ARGS=(python3 "$LB/Meta/record-style-bench.py" "$OUT" --webdriver "$WD" --url "$URL" --timeout 600)
[ -n "$SUITE" ] && ARGS+=(--suite "$SUITE")
exec "${ARGS[@]}"
