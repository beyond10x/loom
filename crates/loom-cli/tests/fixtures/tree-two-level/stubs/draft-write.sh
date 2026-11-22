#!/bin/sh
# Adds a line to the draft; sent back, keeps the feedback document it was given.
echo "draft" >> draft.txt
if [ -n "$LOOM_RETRY_FEEDBACK" ]; then
  cp "$LOOM_RETRY_FEEDBACK" feedback.json
fi
