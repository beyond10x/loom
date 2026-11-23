#!/bin/sh
# Writes its own process id to pid.txt, then sleeps 30 s as that same process.
echo $$ > pid.txt
exec sleep 30
