#!/bin/sh
# Moves itself into the cgroup directory $1, made when absent, and runs the rest of its arguments
# there in its place.
set -eu
mkdir -p "$1"
echo $$ > "$1/cgroup.procs"
shift
exec "$@"
