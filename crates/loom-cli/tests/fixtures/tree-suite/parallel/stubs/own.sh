#!/bin/sh
# Sleeps by the name in tag.txt (late 0.9 s, middle 0.5 s, early 0.1 s), so cycles started together
# end in reverse suite order; copies tag.txt to own.txt; lists the markers of the cycles that
# ended before it into seen.txt; then writes its own marker into the directory barrier/dir.txt
# names.
tag=$(cat tag.txt)
case "$tag" in
  late) sleep 0.9 ;;
  middle) sleep 0.5 ;;
  *) sleep 0.1 ;;
esac
cp tag.txt own.txt
dir=$(cat barrier/dir.txt)
ls "$dir" > seen.txt
: > "$dir/$tag"
