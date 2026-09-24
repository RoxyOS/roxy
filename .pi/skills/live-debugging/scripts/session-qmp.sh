#!/bin/sh
# Resolve the QMP socket for the selected live-debug session.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ -n "${QMP_SOCK:-}" ]; then
    printf '%s\n' "$QMP_SOCK"
    exit 0
fi

if [ -n "${ROXY_DEBUG_SESSION:-}" ]; then
    printf '%s/qmp.sock\n' "$ROXY_DEBUG_SESSION"
    exit 0
fi

session_root=$(CDPATH= cd "$script_dir/../../../../target/roxy/agent-debug" && pwd)
latest=$(ls -dt "$session_root"/run-* 2>/dev/null | head -n 1 || true)
if [ -n "$latest" ]; then
    printf '%s/qmp.sock\n' "$latest"
    exit 0
fi

echo "debug session not found; set QMP_SOCK or ROXY_DEBUG_SESSION" >&2
exit 1
