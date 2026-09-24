#!/bin/sh
# Resolve the QMP socket for the selected live-debug session.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

validate_session() {
    session=$1
    manifest=$session/manifest.json
    [ -f "$manifest" ] || return 1

    pid=$(jq -er '.pid | numbers' "$manifest" 2>/dev/null) || return 1
    qmp=$(jq -er '.qmp | strings' "$manifest" 2>/dev/null) || return 1
    kill -0 "$pid" 2>/dev/null || return 1

    args=$(ps -p "$pid" -o args= 2>/dev/null) || return 1
    case "$args" in
        *"unix:$qmp,"*)
            printf '%s\n' "$qmp"
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

if [ -n "${QMP_SOCK:-}" ]; then
    printf '%s\n' "$QMP_SOCK"
    exit 0
fi

if [ -n "${ROXY_DEBUG_SESSION:-}" ]; then
    if validate_session "$ROXY_DEBUG_SESSION"; then
        exit 0
    fi
    echo "debug session is not an active QEMU VM: $ROXY_DEBUG_SESSION" >&2
    exit 1
fi

session_root=$(CDPATH= cd "$script_dir/../../../../target/roxy/agent-debug" && pwd)
candidates=$(find "$session_root" -mindepth 1 -maxdepth 1 -type d -name 'run-*' \
    -printf '%T@ %p\n' 2>/dev/null | sort -nr | cut -d' ' -f2- || true)
while IFS= read -r candidate; do
    [ -n "$candidate" ] || continue
    if validate_session "$candidate"; then
        exit 0
    fi
done <<EOF
$candidates
EOF

echo "no active debug session found; set QMP_SOCK or ROXY_DEBUG_SESSION" >&2
exit 1
