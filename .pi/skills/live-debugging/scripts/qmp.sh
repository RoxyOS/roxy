#!/bin/sh
# Send one raw QMP JSON request to the agent-debug VM and print the replies.
# Usage: <skill-dir>/scripts/qmp.sh '{"execute":"query-status"}'
# The socket defaults to target/roxy/agent-debug/qmp.sock; override with QMP_SOCK.
set -eu
export QMP_SOCK="${QMP_SOCK:-target/roxy/agent-debug/qmp.sock}"
request="${1:?usage: <skill-dir>/scripts/qmp.sh '<QMP JSON request>'}"
export _QMP_REQUEST="$request"
timeout 5 sh -c '{ printf "{\"execute\":\"qmp_capabilities\"}\n"; sleep 0.1; printf "%s\n" "$_QMP_REQUEST"; sleep 0.2; } | nc -U "$QMP_SOCK"'
