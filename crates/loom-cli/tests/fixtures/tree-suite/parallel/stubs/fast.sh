#!/bin/sh
# Writes the `fast` marker into the directory barrier/dir.txt names, as its last act.
: > "$(cat barrier/dir.txt)/fast"
