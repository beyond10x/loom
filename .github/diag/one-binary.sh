#!/bin/sh
# Diagnosis only (not committed to a product branch): fold every crate's tests/*.rs into one
# test target tests/main.rs, each former file a module loaded by #[path], so its own `mod x;`
# declarations still resolve beside it. Run from the workspace root.
set -eu
for dir in crates/*/; do
  dir=${dir%/}
  count=$(find "$dir/tests" -maxdepth 1 -name '*.rs' 2>/dev/null | wc -l)
  [ "$count" -gt 1 ] || continue
  main="$dir/tests/main.rs"
  [ -f "$main" ] || printf '//! One test binary for the crate (diagnosis).\n' > "$main"
  for file in "$dir"/tests/*.rs; do
    name=$(basename "$file" .rs)
    [ "$name" = main ] && continue
    if [ -d "$dir/tests/$name" ]; then
      printf 'mod %s;\n' "$name" >> "$main"
    else
      printf '#[path = "%s.rs"]\nmod %s;\n' "$name" "$name" >> "$main"
    fi
  done
  manifest="$dir/Cargo.toml"
  awk 'BEGIN{done=0} {print} /^\[package\]/ && !done {print "autotests = false"; done=1}' "$manifest" > "$manifest.new"
  mv "$manifest.new" "$manifest"
  printf '\n[[test]]\nname = "main"\npath = "tests/main.rs"\n' >> "$manifest"
  echo "$dir: $count files"
done
