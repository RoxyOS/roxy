# Keyboard Input into the Live Roxy VM

The framebuffer shell reads from `/dev/tty` via **PS/2 → `roxy-keyboard-input` → tty**. Type into it by injecting
virtual keyboard events into QEMU; `serial.log` is output-only.

## Preferred: QMP `input-send-event`

A key needs one **down** and one **up** event. Always send both, or the key sticks.

```sh
# press and release 'a'
<skill-dir>/scripts/qmp.sh '{"execute":"input-send-event","arguments":{"events":[{"type":"key","data":{"down":true,"key":{"type":"qcode","data":"a"}}}]}}'
<skill-dir>/scripts/qmp.sh '{"execute":"input-send-event","arguments":{"events":[{"type":"key","data":{"down":false,"key":{"type":"qcode","data":"a"}}}]}}'
```

## Alternative: HMP `sendkey` (via scripts)

Types a key including its release — simplest for words:

```sh
<skill-dir>/scripts/hmc.sh 'sendkey l'          # single key
<skill-dir>/scripts/hmc.sh 'sendkey l s ret'    # space-separated list
```

## Text input

Use `type-text.sh` to send a string as one QMP keyboard-event batch. It supports ASCII letters,
digits, spaces, and US-layout punctuation; `--enter` appends Enter. Unsupported characters are
rejected before any events are sent.

```sh
<skill-dir>/scripts/type-text.sh --enter 'X -retro'
```

This injects actual key down/up events through the guest keyboard stack. It does not write to the
serial console or bypass the guest input system.

## Common qcodes

Letters/digits `a`…`z`, `0`…`9`; `ret`, `spc`, `tab`, `esc`, `backspace`, `delete`; `shift`,
`ctrl`, `alt`; arrows/`home`/`end`/`pgup`/`pgdn`; keypad `kp_enter`…; `minus`, `equal`,
`bracket_left`/`right`, `backslash`, `semicolon`, `apostrophe`, `grave_accent`, `comma`, `dot`,
`slash`. Shifted symbols: `sendkey shift-3` for `#` (or shift down/key/shift up via
`input-send-event`).

## Verifying

Guest response is only on the framebuffer, not serial. Type a known-echo command (`echo hello`)
and screenshot it; QMP's return only proves QEMU accepted the events.