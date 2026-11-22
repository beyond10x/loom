#!/bin/sh
# The handoff_when check of route.write, and a writer that shows what it was given; the first
# argument says which.
case "$1" in
  exit)
    # Counts its runs and exits with the status in check-exit (1 when there is none).
    echo run >> check-runs.txt
    exit "$(cat check-exit 2>/dev/null || echo 1)"
    ;;
  env)
    # The environment it was started with, one variable a line, then no handoff.
    tr '\000' '\n' < "/proc/$$/environ"
    exit 1
    ;;
  writer-env)
    # The writer: the environment it was started with, then one more stop.
    tr '\000' '\n' < "/proc/$$/environ"
    echo stop >> route.txt
    ;;
  write)
    # Tries to write the workspace and its target directory, then exits 3.
    echo wrote > check-wrote.txt
    echo wrote > target/check-wrote.txt
    exit 3
    ;;
  sleep)
    sleep 30
    ;;
  signal)
    kill -9 $$
    ;;
esac
