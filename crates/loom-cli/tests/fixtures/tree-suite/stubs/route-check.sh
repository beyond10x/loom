#!/bin/sh
# Valid once the route holds two stops; prints a canary, and keeps it in check.log, which
# no summary may show.
sleep 0.3
echo "CANARY-c4f1e9-route-check"
echo "CANARY-c4f1e9-route-check" >> check.log
test -f route.txt || exit 1
test "$(wc -l < route.txt)" -ge 2
