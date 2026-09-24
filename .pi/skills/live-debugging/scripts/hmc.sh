#!/bin/sh
# Run one HMP command line on the agent-debug VM via QMP human-monitor-command.
# Usage: <skill-dir>/scripts/hmc.sh 'sendkey ret'
# The socket defaults to the latest agent-debug session; override with QMP_SOCK or
# ROXY_DEBUG_SESSION.
set -eu
command_line="${1:?usage: <skill-dir>/scripts/hmc.sh '<HMP command line>'}"
request=$(jq -cn --arg command_line "$command_line" \
    '{execute:"human-monitor-command",arguments:{"command-line":$command_line}}')
exec "$(dirname "$0")/qmp.sh" "$request"
