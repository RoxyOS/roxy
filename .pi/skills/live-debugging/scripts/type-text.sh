#!/usr/bin/env bash
# Type ASCII text into the framebuffer VM through one QMP connection.
# Usage: type-text.sh [--enter] [--delay seconds] 'text to type'
set -euo pipefail

enter=false
delay=0.05
while [[ $# -gt 0 ]]; do
    case "$1" in
        --enter)
            enter=true
            shift
            ;;
        --delay)
            if [[ $# -lt 2 ]]; then
                echo "type-text.sh: --delay requires a value" >&2
                exit 2
            fi
            delay=$2
            shift 2
            ;;
        --)
            shift
            break
            ;;
        -*)
            echo "type-text.sh: unknown option: $1" >&2
            exit 2
            ;;
        *)
            break
            ;;
    esac
done
if [[ $# != 1 ]]; then
    echo "usage: $0 [--enter] [--delay seconds] 'text to type'" >&2
    exit 2
fi
text=$1
qmp_sock=${QMP_SOCK:-target/roxy/agent-debug/qmp.sock}

map_char() {
    local char=$1
    shift_key=false
    case "$char" in
        [a-z]) code=$char ;;
        [A-Z]) code=${char,,}; shift_key=true ;;
        [0-9]) code=$char ;;
        ' ') code=spc ;;
        '-') code=minus ;;
        '=') code=equal ;;
        '[') code=bracket_left ;;
        ']') code=bracket_right ;;
        '\\') code=backslash ;;
        ';') code=semicolon ;;
        "'") code=apostrophe ;;
        '`') code=grave_accent ;;
        ',') code=comma ;;
        '.') code=dot ;;
        '/') code=slash ;;
        '!') code=1; shift_key=true ;;
        '@') code=2; shift_key=true ;;
        '#') code=3; shift_key=true ;;
        '$') code=4; shift_key=true ;;
        '%') code=5; shift_key=true ;;
        '^') code=6; shift_key=true ;;
        '&') code=7; shift_key=true ;;
        '*') code=8; shift_key=true ;;
        '(') code=9; shift_key=true ;;
        ')') code=0; shift_key=true ;;
        '_') code=minus; shift_key=true ;;
        '+') code=equal; shift_key=true ;;
        '{') code=bracket_left; shift_key=true ;;
        '}') code=bracket_right; shift_key=true ;;
        '|') code=backslash; shift_key=true ;;
        ':') code=semicolon; shift_key=true ;;
        '"') code=apostrophe; shift_key=true ;;
        '~') code=grave_accent; shift_key=true ;;
        '<') code=comma; shift_key=true ;;
        '>') code=dot; shift_key=true ;;
        '?') code=slash; shift_key=true ;;
        *) printf 'type-text.sh: unsupported character: %q\n' "$char" >&2; return 2 ;;
    esac
}

validate_text() {
    local char
    for ((i = 0; i < ${#text}; i++)); do
        char=${text:i:1}
        map_char "$char"
    done
}

emit_requests() {
    local char events
    printf '%s\n' '{"execute":"qmp_capabilities"}'

    for ((i = 0; i < ${#text}; i++)); do
        char=${text:i:1}
        map_char "$char"

        if $shift_key; then
            events=$(jq -cn --arg code "$code" '[
                {type:"key",data:{down:true,key:{type:"qcode",data:"shift"}}},
                {type:"key",data:{down:true,key:{type:"qcode",data:$code}}},
                {type:"key",data:{down:false,key:{type:"qcode",data:$code}}},
                {type:"key",data:{down:false,key:{type:"qcode",data:"shift"}}}
            ]')
        else
            events=$(jq -cn --arg code "$code" '[
                {type:"key",data:{down:true,key:{type:"qcode",data:$code}}},
                {type:"key",data:{down:false,key:{type:"qcode",data:$code}}}
            ]')
        fi
        jq -cn --argjson events "$events" \
            '{execute:"input-send-event",arguments:{events:$events}}'
        sleep "$delay"
    done

    if $enter; then
        printf '%s\n' '{"execute":"input-send-event","arguments":{"events":[{"type":"key","data":{"down":true,"key":{"type":"qcode","data":"ret"}}},{"type":"key","data":{"down":false,"key":{"type":"qcode","data":"ret"}}}]}}'
    fi
}

validate_text
if [[ ! -S $qmp_sock ]]; then
    echo "type-text.sh: QMP socket not found: $qmp_sock" >&2
    exit 1
fi

emit_requests | socat - UNIX-CONNECT:"$qmp_sock" >/dev/null
