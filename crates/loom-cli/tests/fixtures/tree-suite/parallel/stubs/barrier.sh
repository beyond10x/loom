#!/bin/sh
# The barrier: writes this cycle's marker into the directory barrier/dir.txt names, then polls
# every 0.1 s, for at most 2 s, until the marker of every cycle barrier/expect.txt names exists.
# Writes `met` or `timed out` into barrier.txt and exits 0.
dir=$(cat barrier/dir.txt)
: > "$dir/$(cat name.txt)"
tries=0
while :; do
  met=met
  for other in $(cat barrier/expect.txt); do
    test -e "$dir/$other" || met=
  done
  if test -n "$met"; then
    echo met > barrier.txt
    exit 0
  fi
  if test "$tries" -ge 20; then
    echo "timed out" > barrier.txt
    exit 0
  fi
  sleep 0.1
  tries=$((tries + 1))
done
