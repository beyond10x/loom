#!/bin/sh
# Makes the directory ro with mode 0555, which nothing may be written into; exit 0.
mkdir ro
chmod 0555 ro
