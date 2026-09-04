#!/usr/bin/env bash
# A 20k dump at 189 longhands is 70-200 MB; the committed receipt is its
# header lines plus a digest and row count of the body (the part `cmp`
# gates). The full dump stays next to it under receipts/full/ (gitignored).
#
#   ./scripts/receipt.sh receipts/full/default.cc.txt receipts/default.cc.txt
set -euo pipefail
full="$1"
out="$2"
{
    grep '^#' "$full"
    body_sha=$(grep -v '^#' "$full" | shasum -a 256 | cut -d' ' -f1)
    rows=$(grep -vc '^#' "$full" || true)
    echo "# BODY_ROWS=$rows BODY_SHA256=$body_sha"
} > "$out"
