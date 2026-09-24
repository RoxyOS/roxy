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
session=target/roxy/agent-debug/run-<id>
kill "$(cat "$session/qemu.pid")"   # SIGTERM; escalate to SIGKILL if needed
```

Confirm the session identity before attaching tools. `cargo xagent-debug` prints the session directory
and GDB port, and writes `manifest.json` there; use those values instead of assuming a fixed
`target/roxy/agent-debug/` path or port. The recorded session PID is a small supervisor
that owns the QEMU process, forwards termination, and writes the final `exit_status` file. Verify
that PID command line, profile, ISO, and kernel all refer to the same VM. A session's QMP socket
and serial log are under that session directory, for example:

```sh
session=target/roxy/agent-debug/run-<id>
jq . "$session/manifest.json"
ps -p "$(jq -r .pid "$session/manifest.json")" -o pid=,args=
cat "$(jq -r .exit_status "$session/manifest.json")"  # after the VM exits
```

The helper scripts accept `QMP_SOCK` to select the session's QMP socket. The GDB port is the
`gdb` value in the manifest, and the ELF must match its `profile`.

When waiting for the VM to reach a state — boot, a program running, the screen changing — poll at
most every 5 seconds; never sleep longer.

When a VM disappears unexpectedly, inspect its session's `qemu.log`, `cpu-reset.log`, `serial.log`,
`exit-status`, and QMP status before starting another run. `cpu-reset.log` is the QEMU CPU-reset
diagnostic named in the manifest. A missing QMP socket alone does not distinguish QEMU startup
failure, guest reset, and guest shutdown.

QMP is newline-delimited JSON over a unix socket. Every connection must first send
`qmp_capabilities`, then the requests, each ending with `\n`. HMP commands are wrapped as QMP
`human-monitor-command` requests (raw HMP has readline echo noise).

Two scripts in this directory cover both (both live next to the skill docs; QMP_SOCK
overrides the default socket):

- `<skill-dir>/scripts/qmp.sh '<JSON request>'` — one raw QMP request; selects the newest active session
  unless `QMP_SOCK` or `ROXY_DEBUG_SESSION` is set
- `<skill-dir>/scripts/hmc.sh '<HMP command line>'` — one HMP command line through QMP

```sh
<skill-dir>/scripts/qmp.sh '{"execute":"query-status"}'   # => {"return":{"status":"running",...}}
<skill-dir>/scripts/qmp.sh '{"execute":"stop"}'           # pause
<skill-dir>/scripts/qmp.sh '{"execute":"cont"}'           # resume
<skill-dir>/scripts/hmc.sh 'sendkey ret'                    # type Enter
```

For literal text, use `<skill-dir>/scripts/type-text.sh [--enter] [--delay seconds] 'text'` to
send an ASCII string over one persistent QMP connection. This is preferable to building HMP
`sendkey` sequences by hand. Do not type into `serial.log` — the framebuffer shell's input comes
from injected keyboard events only.

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
- **GDB**: [`gdb.md`](gdb.md) — attach to the endpoint in the session manifest with the ELF
  matching the recorded `profile`.

## Troubleshooting

- **GDB port busy or wrong VM**: read the session `manifest.json`; it contains the dynamically
  allocated endpoint and the PID/paths that must match the QEMU command line.
- **QEMU died at start**: check `qemu.log` and that `OVMF_CODE` is set in the dev shell.
- **Screenshot garbage/absent**: guest hasn't set a framebuffer mode yet; wait for boot.
