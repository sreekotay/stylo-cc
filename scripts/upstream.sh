#!/usr/bin/env bash
# Drive the servo/stylo submodule the way its CI does.
#
#   ./scripts/upstream.sh build|test|release|bench
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STYLO="$ROOT/stylo"
RECEIPTS="$ROOT/receipts"
CMD="${1:-test}"

if [ ! -f "$STYLO/Cargo.toml" ]; then
    echo "stylo/ is empty. Run: git submodule update --init --depth 1" >&2
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
    # rustup's default install location
    if [ -f "$HOME/.cargo/env" ]; then
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
    fi
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo not on PATH. Install Rust: https://rustup.rs" >&2
    exit 1
fi

cd "$STYLO"

case "$CMD" in
    build)
        cargo build --workspace
        ;;
    test)
        cargo test --workspace
        ;;
    release)
        cargo build --release --features servo
        ;;
    bench)
        mkdir -p "$RECEIPTS"
        out="$RECEIPTS/upstream_$(date +%Y_%m_%d).txt"
        {
            echo "# servo/stylo cargo test --release --workspace"
            echo "# Date: $(date +%Y-%m-%d)"
            echo "# Machine: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m)"
            echo "# rustc: $(rustc --version)"
            echo "# stylo: $(git -C "$STYLO" rev-parse --short HEAD) ($(git -C "$STYLO" log -1 --format=%s))"
            echo
            /usr/bin/time -p cargo test --release --workspace
        } 2>&1 | tee "$out"
        echo "wrote $out"
        ;;
    *)
        echo "Usage: $0 build|test|release|bench" >&2
        exit 1
        ;;
esac
