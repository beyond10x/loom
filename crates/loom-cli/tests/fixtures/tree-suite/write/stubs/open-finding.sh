#!/bin/sh
# Sleeps 0.5 s, then writes findings.yaml with one open finding as its last act; exit 0.
sleep 0.5
printf 'findings:\n  - id: f1\n    status: open\n' > findings.yaml
