#!/usr/bin/env bash
# Local Ladybird patch list. The submodule stays pinned; we never commit inside ladybird/.
#   apply    apply patches/ladybird onto a clean checkout (no-op if already applied)
#   refresh  rewrite the patch list from the current ladybird working tree
#   status   pinned SHA vs dirty files
#   drop     reset ladybird to the pinned SHA (keeps Build/ and out/)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LB="$ROOT/ladybird"
PATCHDIR="$ROOT/patches/ladybird"
SERIES="$PATCHDIR/series"

pinned() { git -C "$LB" rev-parse --short HEAD; }

apply_one() {
  local p="$1"
  if git -C "$LB" apply --check "$p" >/dev/null 2>&1; then
    git -C "$LB" apply "$p"
    echo "applied $(basename "$p")"
  elif git -C "$LB" apply --reverse --check "$p" >/dev/null 2>&1; then
    echo "already applied $(basename "$p")"
  else
    echo "cannot apply $(basename "$p") onto $(pinned)" >&2
    exit 1
  fi
}

apply() {
  [ -f "$LB/Meta/ladybird.py" ] || git -C "$ROOT" submodule update --init --depth 1 ladybird
  [ -f "$SERIES" ] || { echo "no $SERIES"; exit 1; }
  while IFS= read -r name || [ -n "$name" ]; do
    case "$name" in
      ''|'#'*) continue ;;
    esac
    apply_one "$PATCHDIR/$name"
  done < "$SERIES"
}

refresh() {
  mkdir -p "$PATCHDIR"
  git -C "$LB" add -A -- . ':!out' ':!out/**'
  git -C "$LB" diff --cached --binary > "$PATCHDIR/stylecc.patch"
  git -C "$LB" reset -q
  printf '%s\n' 'stylecc.patch' > "$SERIES"
  echo "wrote $PATCHDIR/stylecc.patch ($(wc -l < "$PATCHDIR/stylecc.patch") lines) vs $(pinned)"
}

status() {
  echo "ladybird $(pinned) ($(git -C "$LB" log -1 --format='%s'))"
  git -C "$LB" status --short --untracked-files=normal | grep -v '^?? out/' || true
  if [ -f "$SERIES" ]; then
    echo "series:"
    cat "$SERIES"
  fi
}

drop() {
  git -C "$LB" reset --hard HEAD
  git -C "$LB" clean -fd -- Libraries Tests
  echo "ladybird reset to $(pinned)"
}

cmd="${1:-status}"
case "$cmd" in
  apply|refresh|status|drop) "$cmd" ;;
  *) echo "usage: $0 [apply|refresh|status|drop]"; exit 2 ;;
esac
