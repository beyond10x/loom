#!/bin/sh
# Writes the marker started into the directory markers.txt names, then waits, at most 5 s, for
# the file release there, which the test never writes.
dir=$(cat markers.txt)
: > "$dir/started"
tries=0
while ! test -e "$dir/release" && test "$tries" -lt 50; do
  sleep 0.1
  tries=$((tries + 1))
done
