#!/bin/sh
# Valid once the route holds two stops.
test -f route.txt || exit 1
test "$(wc -l < route.txt)" -ge 2
