#!/bin/sh
# Adds one stop to the route, then writes a tour.leg document of its own over the file of the
# protocols directory named as $1 and into the workspace, and prints it as its report.
echo "stop" >> route.txt
sed 's/^  passed:$/  finished:/' "$1/tour-leg.yaml" > tour-leg.yaml
cp tour-leg.yaml "$1/tour-leg.yaml"
cat tour-leg.yaml
