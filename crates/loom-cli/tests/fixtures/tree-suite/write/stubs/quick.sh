#!/bin/sh
# Writes findings.yaml with one open finding at once; exit 0.
printf 'findings:\n  - id: f1\n    status: open\n' > findings.yaml
