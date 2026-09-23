#!/usr/bin/env bash
# Type ASCII text into the framebuffer VM through QMP.
# Usage: type-text.sh [--enter] 'text to type'
set -euo pipefail

enter=false
if [[ ${1:-} == --enter ]]; then
    enter=true
    shift
fi
if [[ $# != 1 ]]; then
    echo "usage: $0 [--enter] 'text to type'" >&2
    exit 2
fi
text=$1

# Send each character separately so the guest input queues can keep up.
for ((i = 0; i < ${#text}; i++)); do
    char=${text:i:1}
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
        *) printf 'type-text.sh: unsupported character: %q\\n' "$char" >&2; exit 2 ;;
    esac

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
    request=$(jq -cn --argjson events "$events" '{execute:"input-send-event",arguments:{events:$events}}')
    "$(dirname "$0")/qmp.sh" "$request" >/dev/null
    sleep 0.05
done

if $enter; then
    "$(dirname "$0")/hmc.sh" 'sendkey ret' >/dev/null
fi
