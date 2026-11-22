#!/bin/sh
# Started under systemd-run --user --scope -p Delegate=yes: moves itself into keeper/ of its
# scope, hands cpu, memory and pids to the scope's children, prints the scope's directory and
# waits until its standard input closes.
set -eu
scope=/sys/fs/cgroup$(sed -n 's/^0:://p' /proc/self/cgroup)
mkdir "$scope/keeper"
echo $$ > "$scope/keeper/cgroup.procs"
echo "+cpu +memory +pids" > "$scope/cgroup.subtree_control"
echo "$scope"
exec cat > /dev/null
