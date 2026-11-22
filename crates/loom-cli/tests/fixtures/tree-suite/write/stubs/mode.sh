#!/bin/sh
# Writes old.txt with mode 0600; exit 0.
printf 'old\n' > old.txt
chmod 0600 old.txt
