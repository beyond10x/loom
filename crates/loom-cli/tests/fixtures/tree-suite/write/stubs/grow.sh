#!/bin/sh
# Appends 1025 KiB to seeds/grows.yaml of the suite directory suite.txt names, so the source of
# the write after it is over 1 MiB; exit 0.
dd if=/dev/zero bs=1024 count=1025 2>/dev/null >> "$(cat suite.txt)/seeds/grows.yaml"
