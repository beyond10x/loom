#!/bin/sh
# Writes target/marker, waits at the gate (stubs/gate.sh), then reads the other cycle's marker by
# its host path (peer/host.txt) and by the path relative to this workspace (peer/relative.txt).
# Writes target/unseen when both reads fail, target/seen naming each read that did not.
: > target/marker
. stubs/gate.sh
seen=
cat "$(cat peer/host.txt)" > /dev/null 2>&1 && seen="$seen host"
cat "$(cat peer/relative.txt)" > /dev/null 2>&1 && seen="$seen relative"
if [ -z "$seen" ]; then
  : > target/unseen
else
  echo "$seen" > target/seen
fi
