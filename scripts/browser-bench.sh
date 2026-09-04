#!/usr/bin/env bash
# Run WebKit's StyleBench in real browsers on this machine and print per-suite ms.
#
#   scripts/browser-bench.sh                 # fetch (if needed), then Chrome + Playwright WebKit, 5 iterations
#   scripts/browser-bench.sh chrome [N]      # installed Google Chrome, headless=new, N iterations
#   scripts/browser-bench.sh webkit [N]      # Playwright's WebKit build (stand-in; not Safari's WebCore)
#   scripts/browser-bench.sh safari          # safaridriver if "Allow Remote Automation" is on, else manual instructions
#   scripts/browser-bench.sh serve           # just serve StyleBench for a manual run
#   scripts/browser-bench.sh fetch           # (re)fetch StyleBench from WebKit main (sparse clone)
#   scripts/browser-bench.sh report FILE     # format a pasted benchmarkClient._measuredValuesList JSON
#
# Extra args after N are passed to browser-bench/run.mjs (e.g. --suite "Sibling combinators", --headed, --json out.json).
# Everything lives under browser-bench/ (node_modules, StyleBench copy, results are gitignored). No sudo, no globals.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BB="$ROOT/browser-bench"
cd "$BB"

fetch() {
  local tmp="$BB/.webkit-sparse"
  rm -rf "$tmp"
  echo "fetching PerformanceTests/StyleBench + PerformanceTests/resources from WebKit main (sparse, blobless clone)..."
  git clone --quiet --depth 1 --filter=blob:none --sparse https://github.com/WebKit/WebKit.git "$tmp"
  git -C "$tmp" sparse-checkout set --quiet PerformanceTests/StyleBench PerformanceTests/resources
  rm -rf "$BB/StyleBench" "$BB/resources"
  cp -R "$tmp/PerformanceTests/StyleBench" "$BB/StyleBench"
  cp -R "$tmp/PerformanceTests/resources" "$BB/resources"
  git -C "$tmp" log -1 --format='%H %cd' > "$BB/StyleBench/UPSTREAM.txt"
  rm -rf "$tmp"
  echo "StyleBench at WebKit $(cat "$BB/StyleBench/UPSTREAM.txt")"
}

deps() {
  command -v node >/dev/null || { echo "node not found (need node >= 18)"; exit 1; }
  [ -d "$BB/node_modules/playwright" ] || npm install --no-fund --no-audit
}

ensure_webkit() {
  # Playwright downloads its WebKit build into ~/Library/Caches/ms-playwright (user cache, no sudo).
  node -e "const {webkit}=require('playwright');require('fs').accessSync(webkit.executablePath())" 2>/dev/null \
    || npx playwright install webkit
}

machine() {
  echo "machine: $(sysctl -n machdep.cpu.brand_string), $(sysctl -n hw.ncpu) cores, macOS $(sw_vers -productVersion)"
  [ -d "/Applications/Google Chrome.app" ] && echo "chrome:  $("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --version)"
  [ -d "/Applications/Safari.app" ] && echo "safari:  $(defaults read /Applications/Safari.app/Contents/Info.plist CFBundleShortVersionString)"
  echo "node:    $(node --version), playwright $(npx playwright --version)"
}

safari() {
  # Playwright cannot drive Safari; safaridriver (WebDriver) can, but only if the user has turned on
  # Develop > Developer Settings > "Allow Remote Automation" (or ran `safaridriver --enable`, which needs sudo; we don't).
  local p=4445
  safaridriver -p $p & local pid=$!
  sleep 2
  local resp
  resp=$(curl -s -X POST "localhost:$p/session" -H 'Content-Type: application/json' -d '{"capabilities":{}}' || true)
  if echo "$resp" | grep -q '"sessionId"'; then
    local sid; sid=$(echo "$resp" | sed -E 's/.*"sessionId":"([^"]+)".*/\1/')
    curl -s -X DELETE "localhost:$p/session/$sid" >/dev/null
    kill $pid 2>/dev/null || true
    node safari-webdriver.mjs --iterations "${1:-5}" "${@:2}"
  else
    kill $pid 2>/dev/null || true
    echo
    echo "safaridriver refused to create a session:"
    echo "  $resp"
    echo
    echo "Safari automation is off. To turn it on (UI, no sudo): Safari > Settings > Advanced > 'Show features for web developers',"
    echo "then Develop > Developer Settings > 'Allow Remote Automation'. Then re-run: scripts/browser-bench.sh safari"
    echo
    echo "Manual fallback: run  scripts/browser-bench.sh serve  and follow the printed instructions."
    echo "Stand-in: scripts/browser-bench.sh webkit  runs Playwright's WebKit build (not Safari's shipped WebCore)."
    return 3
  fi
}

cmd="${1:-all}"; shift || true
case "$cmd" in
  fetch) fetch ;;
  serve) deps; [ -d StyleBench ] || fetch; node run.mjs --serve "$@" ;;
  report) deps; f="$1"; [[ "$f" = /* ]] || f="$ROOT/$f"; node run.mjs --from-json "$f" "${@:2}" ;;
  chrome) deps; [ -d StyleBench ] || fetch; machine; node run.mjs --browser chrome --iterations "${1:-5}" --split "${@:2}" ;;
  chromium) deps; [ -d StyleBench ] || fetch; machine; node run.mjs --browser chromium --iterations "${1:-5}" --split "${@:2}" ;;
  webkit) deps; [ -d StyleBench ] || fetch; ensure_webkit; machine; node run.mjs --browser webkit --iterations "${1:-5}" --split "${@:2}" ;;
  safari) deps; [ -d StyleBench ] || fetch; machine; safari "$@" ;;
  all)
    deps; [ -d StyleBench ] || fetch; ensure_webkit; machine
    mkdir -p results
    node run.mjs --browser chrome --iterations "${1:-5}" --split --json results/chrome.json "${@:2}"
    node run.mjs --browser webkit --iterations "${1:-5}" --split --json results/webkit.json "${@:2}"
    safari "${1:-5}" "${@:2}" || true
    ;;
  *) echo "usage: $0 [all|chrome|chromium|webkit|safari|serve|fetch|report] [iterations] [-- run.mjs args]"; exit 2 ;;
esac
