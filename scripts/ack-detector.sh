#!/bin/bash
# ack-detector — confirms a bead comment ACTUALLY LANDED by reading it back.
#
# THE TRAP (measured 2026-08-31): `br comment` (SINGULAR) prefix-matches to
# `br comments`, prints 'error: unexpected argument' on stderr, and may exit 0
# or 2 depending on the argument shape. The comment does NOT land in either
# case. An agent that trusts the exit code believes the comment landed.
#
# THE CONTRACT: an ack is confirmed by READ-BACK — run `br comments list <id>`
# and grep for a unique marker from the comment. Exit 0 from the posting
# command is necessary but not sufficient.
#
# USAGE: ack-detector.sh <bead-id> <unique-marker>
# EXIT CODES:
#   0 = ACK_CONFIRMED (the marker appears in the bead's comment list)
#   1 = ACK_MISSING   (the marker does not appear — the comment did not land)
#   2 = USAGE ERROR   (missing arguments)
#   3 = TRACKER UNREADABLE (br comments list failed — not 'no ack')

set -uo pipefail

BEAD_ID="${1:-}"
MARKER="${2:-}"

if [ -z "$BEAD_ID" ] || [ -z "$MARKER" ]; then
    echo "usage: ack-detector.sh <bead-id> <unique-marker>" >&2
    exit 2
fi

# READ-BACK: the only surface that proves the comment landed.
# br comments (PLURAL) list <id> — not br comment (SINGULAR).
readback=$(br comments list "$BEAD_ID" 2>&1)
readback_rc=$?

if [ "$readback_rc" -ne 0 ]; then
    echo "ACK_UNVERIFIABLE: br comments list $BEAD_ID exited $readback_rc — the tracker is unreadable, which is an ERROR, not 'no ack'" >&2
    exit 3
fi

if echo "$readback" | grep -qF "$MARKER"; then
    echo "ACK_CONFIRMED: marker found in $BEAD_ID comments"
    exit 0
else
    echo "ACK_MISSING: marker '$MARKER' not found in $BEAD_ID comments — the comment did not land" >&2
    exit 1
fi
