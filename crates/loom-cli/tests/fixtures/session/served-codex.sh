#!/bin/sh
if [ "$1" = "--version" ]; then echo "codex-cli 0.153.4"; exit; fi
echo ran >> '@SEEN@'
SESSION="$CODEX_HOME/sessions/2026/08/22"
/bin/mkdir -p "$SESSION"
ROLLOUT="$SESSION/rollout-fixture.jsonl"
printf '%s\n' "{\"timestamp\":\"2026-08-22T10:00:00.000Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"codex-1\",\"session_id\":\"codex-1\",\"cli_version\":\"0.153.4\",\"cwd\":\"$PWD\",\"originator\":\"codex_exec\",\"model_provider\":\"openai\"}}" > "$ROLLOUT"
printf '%s\n' '{"timestamp":"2026-08-22T10:00:01.000Z","type":"event_msg","payload":{"type":"task_started","turn_id":"t1"}}' >> "$ROLLOUT"
printf '%s\n' '{"timestamp":"2026-08-22T10:00:05.000Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"t1","duration_ms":4200,"error":null,"last_agent_message":"{\"action\":\"session.propose\",\"summary\":\"Codex fixture reviewed the file\"}"}}' >> "$ROLLOUT"
