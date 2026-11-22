#!/bin/sh
# Keeps the cgroup it runs in, says it started, then waits until target/release exists (at most
# 30 s) and ends 0.
cat /proc/self/cgroup > target/cgroup.txt
: > target/started
n=0
while [ ! -e target/release ]; do
  n=$((n + 1))
  [ "$n" -le 300 ] || exit 7
  sleep 0.1
done
