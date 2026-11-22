#!/bin/sh
# Replaces seeds/relinks.yaml of the suite directory suite.txt names with a link to the ledger
# seed beside it; exit 0.
dir=$(cat suite.txt)
rm -f "$dir/seeds/relinks.yaml"
ln -s "$dir/seeds/ledger.txt" "$dir/seeds/relinks.yaml"
