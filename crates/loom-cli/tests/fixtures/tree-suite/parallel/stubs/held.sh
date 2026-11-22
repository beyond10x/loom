#!/bin/sh
# Writes this cycle's started marker into the directory barrier/dir.txt names, then waits, at
# most 5 s, for the file `release` there.
dir=$(cat barrier/dir.txt)
: > "$dir/$(cat name.txt)"
tries=0
while ! test -e "$dir/release" && test "$tries" -lt 50; do
  sleep 0.1
  tries=$((tries + 1))
done
