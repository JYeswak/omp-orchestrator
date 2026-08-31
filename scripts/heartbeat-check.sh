#!/bin/bash
# heartbeat-check — verifies the omp-orchestrator supervisor's heartbeat is FRESH.
#
# This script is INDEPENDENT of the supervisor: it reads the heartbeat file the
# supervisor writes and escalates if the heartbeat is stale. A supervisor whose
# liveness nobody verifies is the same defect one level up — tick-monitor ran
# 7h28m with 0 restarts, correctly detected 4 idle panes for 178 consecutive
# ticks, and nothing consumed its alarm because the only reader of the output
# was the process that wrote it.
#
# THE CONTRACT:
#   The supervisor writes heartbeat.json every 60 seconds containing:
#     { "ts": <unix_epoch>, "pid": <int>, "session": "<name>", "repo": "<path>",
#       "decision": "<SupervisorDecision variant>" }
#   This checker runs every 5 minutes via cron (or a separate launchd agent).
#   If heartbeat.json is missing, unparseable, or older than 180 seconds
#   (3× the heartbeat interval, allowing one missed cycle), the checker
#   ESCALATES: it prints a typed error to stderr and exits nonzero.
#
# EXIT CODES:
#   0 = heartbeat fresh, supervisor alive
#   1 = heartbeat STALE (supervisor may be dead or hung)
#   2 = heartbeat MISSING (supervisor never started or state dir deleted)
#   3 = heartbeat UNPARSEABLE (corrupted — investigate the disk)
#   4 = usage error
#
# USAGE: heartbeat-check [--max-age-secs 180] [--heartbeat-path PATH]
#   Defaults: --max-age-secs 180, --heartbeat-path ~/.local/state/omp-orchestrator/heartbeat.json

set -uo pipefail

MAX_AGE=180
HB_PATH="${HOME:-/Users/josh}/.local/state/omp-orchestrator/heartbeat.json"

while [ $# -gt 0 ]; do
    case "$1" in
        --max-age-secs) MAX_AGE="$2"; shift 2 ;;
        --heartbeat-path) HB_PATH="$2"; shift 2 ;;
        *) echo "usage: heartbeat-check [--max-age-secs N] [--heartbeat-path PATH]" >&2; exit 4 ;;
    esac
    shift 2>/dev/null || shift $(( $# > 0 ? 1 : 0 ))
done 2>/dev/null

if [ ! -f "$HB_PATH" ]; then
    echo "HEARTBEAT_MISSING: $HB_PATH does not exist — the supervisor never started or the state dir was deleted" >&2
    exit 2
fi

now=$(date +%s)
hb_mtime=$(stat -f %m "$HB_PATH" 2>/dev/null) || { echo "HEARTBEAT_MISSING: cannot stat $HB_PATH" >&2; exit 2; }
age=$((now - hb_mtime))

if [ "$age" -gt "$MAX_AGE" ]; then
    echo "HEARTBEAT_STALE: $HB_PATH last modified ${age}s ago (max ${MAX_AGE}s) — the supervisor may be dead or hung" >&2
    exit 1
fi

# Parse the heartbeat content: the supervisor writes JSON with ts, pid, session, repo, decision.
content=$(cat "$HB_PATH")
if ! echo "$content" | python3 -c "import json,sys; json.load(sys.stdin)" 2>/dev/null; then
    echo "HEARTBEAT_UNPARSEABLE: $HB_PATH is not valid JSON — investigate the disk" >&2
    exit 3
fi

hb_ts=$(echo "$content" | python3 -c "import json,sys; print(json.load(sys.stdin).get('ts', 0))" 2>/dev/null)
content_age=$((now - hb_ts))
if [ "$content_age" -gt "$MAX_AGE" ]; then
    echo "HEARTBEAT_STALE_CONTENT: embedded ts is ${content_age}s old (file mtime is fresh but the CONTENT is stale — the supervisor may be writing but not updating)" >&2
    exit 1
fi

session=$(echo "$content" | python3 -c "import json,sys; print(json.load(sys.stdin).get('session',''))" 2>/dev/null)
decision=$(echo "$content" | python3 -c "import json,sys; print(json.load(sys.stdin).get('decision',''))" 2>/dev/null)
echo "HEARTBEAT_OK: session=$session decision=$decision age=${age}s"
exit 0
