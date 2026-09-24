#!/bin/sh
# Send one raw QMP JSON request to the agent-debug VM and print the replies.
# Usage: <skill-dir>/scripts/qmp.sh '{"execute":"query-status"}'
# The socket defaults to the latest agent-debug session; override with QMP_SOCK or
# ROXY_DEBUG_SESSION.
set -eu
export QMP_SOCK="${QMP_SOCK:-$("$(dirname "$0")/session-qmp.sh")}"
request="${1:?usage: <skill-dir>/scripts/qmp.sh '<QMP JSON request>'}"

# QEMU's `server,nowait` re-creates the listening socket after each disconnect.
# socat is fast enough to hit the brief window before the new listener is ready,
# so retry a few times with a short back-off.
attempt=1
while true; do
    if printf '{"execute":"qmp_capabilities"}\n%s\n' "$request" | socat - UNIX-CONNECT:"$QMP_SOCK" 2>/dev/null; then
        exit 0
    fi
    if [ "$attempt" -ge 5 ]; then
        echo "qmp.sh: failed to connect to $QMP_SOCK after $attempt attempts" >&2
        exit 1
    fi
    attempt=$((attempt + 1))
    sleep 0.05
done