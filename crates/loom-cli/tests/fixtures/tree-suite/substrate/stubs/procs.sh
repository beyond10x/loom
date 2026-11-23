#!/bin/sh
# Bound with the argv marker TOKEN-ONE or TOKEN-TWO ($1). Waits at the gate (stubs/gate.sh).
# TOKEN-ONE then waits for target/done (at most 10 s). TOKEN-TWO reads every /proc/<pid>/cmdline
# it can see, in this shell alone (no command whose argv names a token), and writes
# target/procs.txt: `self: seen` when one holds TOKEN-TWO, and `other: seen` or `other: unseen`
# for TOKEN-ONE.
. stubs/gate.sh
if [ "$1" = TOKEN-ONE ]; then
  tries=0
  while [ ! -e target/done ]; do
    tries=$((tries + 1))
    [ "$tries" -le 100 ] || exit 7
    sleep 0.1
  done
  exit 0
fi
self=unseen
other=unseen
for cmdline in /proc/[0-9]*/cmdline; do
  line=$(tr '\000' ' ' < "$cmdline" 2> /dev/null) || continue
  case "$line" in
    *TOKEN-TWO*) self=seen ;;
  esac
  case "$line" in
    *TOKEN-ONE*) other=seen ;;
  esac
done
printf 'self: %s\nother: %s\n' "$self" "$other" > target/procs.txt
