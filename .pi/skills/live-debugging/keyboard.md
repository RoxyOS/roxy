# Keyboard Input into the Live Roxy VM

In this kernel the interactive terminal's keyboard path is **PS/2 → evdev → `/dev/tty`**. The
framebuffer shell (`/bin/sh`) reads from `/dev/tty`, so the only way to type into it is to inject
virtual keyboard events; typing into `serial.log` has no effect (serial is a diagnostics *output*
only). Those events are injected with QEMU's virtual keyboard, which the guest's PS/2 controller
sees as a normal keyboard.

## Preferred: QMP `input-send-event`

Emits a discrete key. A key needs one **down** event and one **up** event (QEMU does not
auto-repeat unless you send hold/repeat). `qcode` names are the conventional QEMU key names
(`a`, `1`, `ret`, `spc`, `shift`…).

```sh
SO=.../target/roxy/agent-debug/qmp.sock
# press and release 'a'
timeout 5 bash -c "{ printf '{\"execute\":\"qmp_capabilities\"}\\n'; sleep 0.1; \
      printf '{\"execute\":\"input-send-event\",\"arguments\":{\\\"events\\\":[{\\\"type\\\":\\\"key\\\",\\\"data\\\":{\\\"down\\\":true,\\\"key\\\":{\\\"type\\\":\\\"qcode\\\",\\\"data\\\":\\\"a\\\"}}}]}}\\n'; sleep 0.1; \
      printf '{\"execute\":\"input-send-event\",\"arguments\":{\\\"events\\\":[{\\\"type\\\":\\\"key\\\",\\\"data\\\":{\\\"down\\\":false,\\\"key\\\":{\\\"type\\\":\\\"qcode\\\",\\\"data\\\":\\\"a\\\"}}}]}}\\n'; sleep 0.2; \
      } | nc -U $SO"
```

Both events returning `{"return": {}}` means QEMU accepted them. **Always send the up event** for
every down event, or the key sticks and subsequent input is corrupted.

## Alternative: HMP `sendkey` (via QMP to avoid echo noise)

`sendkey` types a whole key with its release built in, so it is the simplest way to type a word:

```sh
# type "ls" then Enter
for k in l s ret; do
  timeout 5 bash -c "{ printf '{\"execute\":\"qmp_capabilities\"}\\n'; sleep 0.1; \
        printf '{\"execute\":\"human-monitor-command\",\"arguments\":{\\\"command-line\\\":\\\"sendkey $k\\\"}}\\n'; \
        sleep 0.15; } | nc -U $SO"
done
```

`sendkey` with a single key string accepts a space-separated list (`sendkey l s ret`). A trailing
`hold_ms` overrides the default 100 ms hold. Using `human-monitor-command` avoids the raw HMP
socket's readline echo noise.

## Common `qcode` names (verified available)

- Letters/digits: `a`…`z`, `0`…`9`
- `ret` (Enter), `spc`, `tab`, `esc`, `backspace`, `delete`
- `shift`, `ctrl`, `alt`, `ctrl_r`, `alt_r`
- `up`, `down`, `left`, `right`, `home`, `end`, `pgup`, `pgdn`
- `kp_enter`, `kp_add`… (keypad), `minus`, `equal`, `bracket_left`, `bracket_right`,
  `backslash`, `semicolon`, `apostrophe`, `grave_accent`, `comma`, `dot`, `slash`

For shifted symbols, e.g. `#`, use `shift-3`: `sendkey shift-3` (or in `input-send-event`, send a
`shift` down + key down + key up + `shift` up). Refer to QEMU's key list (`query-keys` via QMP,
or the project HMP `/help sendkey`) for the canonical set.

## Verifying the input

The guest's response is only visible on the framebuffer, **not** in `serial.log`. Type a command
and capture a screenshot (`screenshot.md`), or observe `input-send-event`'s success return. If the
shell has a prompt, a known-echo command (`echo hello`) is a good probe.

## Notes

- There is a slight timing latency per `nc -U` connection. For fast, ordered typing prefer one
  held-open stream that sends the `qmp_capabilities` once and then the key events back-to-back.
- Wrapping guards with `timeout` prevents a stale connection from hanging the invoking shell.