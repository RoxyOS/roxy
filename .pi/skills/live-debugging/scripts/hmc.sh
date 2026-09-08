#!/bin/sh
# Run one HMP command line on the agent-debug VM via QMP human-monitor-command.
# Usage: <skill-dir>/scripts/hmc.sh 'sendkey ret'
# The socket defaults to target/roxy/agent-debug/qmp.sock; override with QMP_SOCK.
set -eu
command_line="${1:?usage: <skill-dir>/scripts/hmc.sh '<HMP command line>'}"
export _HMC_COMMAND="$command_line"
exec "$(dirname "$0")/qmp.sh" "$(printf '{"execute":"human-monitor-command","arguments":{"command-line":"%s"}}' "$_HMC_COMMAND")"
