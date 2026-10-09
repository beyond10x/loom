#!/usr/bin/env bash
# Counts the plugin layer's code: non-blank lines that are not `//` comments, per file and per
# crate. `--files` prints only the measured files, one per line, for the completeness check of
# plugin-layer-measurement.md. Run from the repository root.
set -euo pipefail

files() {
  find crates/loom-plugin/src crates/loom-plugin-slack/src -name '*.rs' | sort
  echo crates/loom-connectors/src/cli.rs
}

count() {
  grep -cvE '^[[:space:]]*($|//)' "$1" || true
}

if [ "${1:-}" = "--files" ]; then
  files
  exit 0
fi

declare -A total
while read -r f; do
  n=$(count "$f")
  case "$f" in
    crates/loom-plugin/*) c=loom-plugin ;;
    crates/loom-plugin-slack/*) c=loom-plugin-slack ;;
    *) c=loom-connectors/cli ;;
  esac
  total[$c]=$(( ${total[$c]:-0} + n ))
  printf '%6d  %s\n' "$n" "$f"
done < <(files)
echo
for c in loom-plugin loom-plugin-slack loom-connectors/cli; do
  printf '%6d  %s (crate total)\n' "${total[$c]}" "$c"
done
