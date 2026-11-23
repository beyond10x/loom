#!/bin/sh
# The host-side gate, sourced by the gated stubs: says the program started (target/started), then
# polls its own target/release every 0.1 s, for at most 10 s; exits 7 when it never came. Makes
# target/ when absent (Substrate makes it; an unconfined runner does not).
mkdir -p target
: > target/started
tries=0
while [ ! -e target/release ]; do
  tries=$((tries + 1))
  [ "$tries" -le 100 ] || exit 7
  sleep 0.1
done
