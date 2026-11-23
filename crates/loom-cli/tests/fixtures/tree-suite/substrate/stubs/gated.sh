#!/bin/sh
# Copies tag.txt to target/own.txt, then waits at the gate (stubs/gate.sh) until the test releases
# it.
cp tag.txt target/own.txt
. stubs/gate.sh
