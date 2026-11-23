#!/bin/sh
# Says it started, then ends itself with SIGKILL, as an out-of-memory or cgroup kill would.
: > target/started
kill -KILL $$
