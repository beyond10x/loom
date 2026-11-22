#!/bin/sh
# Waits, at most 3 s, for the marker the `fast` cycle's program writes as its last act, writes
# `met` or `timed out` into waited.txt, then sleeps 1 s more, so it ends after `fast`.
dir=$(cat barrier/dir.txt)
tries=0
while ! test -e "$dir/fast" && test "$tries" -lt 30; do
  sleep 0.1
  tries=$((tries + 1))
done
if test -e "$dir/fast"; then
  echo met > waited.txt
else
  echo "timed out" > waited.txt
fi
sleep 1
