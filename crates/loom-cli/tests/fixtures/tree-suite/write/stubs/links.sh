#!/bin/sh
# Makes two links in the workspace: out to the file f of the directory outside.txt names, and
# up to the directory above the workspace; exit 0.
ln -s "$(cat outside.txt)/f" out
ln -s .. up
