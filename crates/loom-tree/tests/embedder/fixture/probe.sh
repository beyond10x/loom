#!/bin/sh
# Writes the cgroup it runs in to its standard output, which the tree keeps as the run's blob.
cat /proc/self/cgroup
