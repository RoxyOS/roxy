---
name: live-debugging
description: Use when you need to debug a live qemu vm session, attaching gdb, sending key inputs, mouse inputs, and screendumping.
---

# Live QEMU Debugging in Roxy OS

`cargo xrun` cannot be driven by an agent (serial on the terminal, `-monitor none`). Instead use
`cargo xagent-debug`, which boots the same kernel in QEMU **detached** with every control channel
exposed under `target/roxy/agent-debug/`.

## Starting and stopping

```sh
cargo xagent-debug --profile dev      # source-level GDB (DWARF)
cargo xagent-debug --profile release  # optimized, symbols only
```

`--profile` is mandatory. The VM boots to `bash-5.3#` unpaused; to catch early boot, pause right
after start (below) before attaching GDB.

```sh
kill "$(cat target/roxy/agent-debug/qemu.pid)"   # SIGTERM; escalate to SIGKILL if needed
```

Confirm `tcp:1234` is free before relaunching (a stale instance holds it).

When waiting for the VM to reach a state — boot, a program running, the screen changing — poll at
most every 5 seconds; never sleep longer.

## Speaking QMP and HMP

QMP is newline-delimited JSON over a unix socket. Every connection must first send
`qmp_capabilities`, then the requests, each ending with `\n`. HMP commands are wrapped as QMP
`human-monitor-command` requests (raw HMP has readline echo noise).

Two scripts in this directory cover both (both live next to the skill docs; QMP_SOCK
overrides the default socket):

- `<skill-dir>/scripts/qmp.sh '<JSON request>'` — one raw QMP request
- `<skill-dir>/scripts/hmc.sh '<HMP command line>'` — one HMP command line through QMP

```sh
<skill-dir>/scripts/qmp.sh '{"execute":"query-status"}'   # => {"return":{"status":"running",...}}
<skill-dir>/scripts/qmp.sh '{"execute":"stop"}'           # pause
<skill-dir>/scripts/qmp.sh '{"execute":"cont"}'           # resume
<skill-dir>/scripts/hmc.sh 'sendkey ret'                    # type Enter
```

For literal text, use `<skill-dir>/scripts/type-text.sh [--enter] 'text'` to send an ASCII string
as one QMP key-event batch. This is preferable to building HMP `sendkey` sequences by hand. Do not
type into `serial.log` — the framebuffer shell's input comes from injected keyboard events only.

QMP event types (all strings, verified against QEMU 11):

- `key` + `key:"qcode"` — keyboard; each key needs a down and an up event
- `rel` + `axis:"x"/"y"` — relative pointer movement
- `btn` + `button:"left"/"right"/"middle"` — pointer button down/up
- `abs` is not handled by this machine's PS/2 input

HMP input equivalents (`<skill-dir>/scripts/hmc.sh`): `sendkey`, `mouse_move dx dy`, `mouse_button state`
(mask `1`=L `2`=R `4`=M, `0`=released).

## Channel-specific guides

- **Screen**: [`screenshot.md`](screenshot.md) — `screendump` a PNG via QMP; framebuffer is
  1280x800.
- **Keyboard**: [`keyboard.md`](keyboard.md) — QMP `input-send-event` (key down+up) or HMP
  `sendkey`; guest path is PS/2 → `roxy-keyboard-input` → tty.
- **Mouse**: [`mouse.md`](mouse.md) — QMP relative/button events or HMP `mouse_move`/`mouse_button`;
  relative motion only, track the cursor yourself.
- **GDB**: [`gdb.md`](gdb.md) — attach to `tcp:1234` with the ELF matching the `--profile` you
  launched.

## Troubleshooting

- **`tcp:1234` busy**: a previous instance is still running; kill via pidfile or
  `pgrep -af qemu-system`.
- **QEMU died at start**: check `qemu.log` and that `OVMF_CODE` is set in the dev shell.
- **Screenshot garbage/absent**: guest hasn't set a framebuffer mode yet; wait for boot.
