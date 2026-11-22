#!/bin/sh
# Applies the decision findings.yaml holds: exit 0 when its finding is decided, else 1.
grep -q 'status: decided' findings.yaml
