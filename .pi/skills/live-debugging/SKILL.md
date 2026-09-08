---
name: live-debugging
description: "Use when running Roxy OS inside QEMU and interacting with the live virtual machine — taking a screenshot of the framebuffer, injecting keyboard input, moving/clicking the virtual mouse, or attaching GDB to the running kernel. Covers the `cargo xagent-debug` channel set (serial log, QMP socket, monitor socket, GDB stub)."
---

# Live QEMU Debugging in Roxy OS

Roxy OS boots from a Limine ISO under OVMF/EFI onto a q35 machine and reaches a framebuffer
`/bin/sh` running as init. To debug a *running* instance you need to observe and drive the
virtual machine, not just read static code. This skill documents the single command that exposes
every control channel and how to use each one.

Always prefer this tooling over the interactive `cargo xrun`. `xrun` attaches the serial console
to the invoking terminal and disables the monitor (`-monitor none`), so it cannot be driven by an
agent. `cargo xagent-debug` instead starts QEMU detached with stable sockets, a serial log file,
and a GDB stub.

## Getting a debuggable VM

The development shell provides `qemu-system-x86_64`, GDB, `nc`, `jq`, and the OVMF firmware.
Run, from the workspace root:

```sh
cargo xagent-debug --profile dev   # source-level GDB (DWARF)
cargo xagent-debug --profile release  # optimized, symbol-only
```

`--profile` is required. `dev` builds the kernel unoptimized with DWARF debug info, so GDB can
resolve source lines; `release` is the optimized kernel (symbols but no DWARF, see `gdb.md`).

This builds the selected kernel, crafts the ISO, then launches QEMU **detached** and prints the
channel endpoints under `target/roxy/agent-debug/`:

| Channel | Path / endpoint | Purpose |
|---|---|---|
| Serial log | `target/roxy/agent-debug/serial.log` | Kernel/boot diagnostics (tee'd from framebuffer) |
| QMP socket | `target/roxy/agent-debug/qmp.sock` | JSON control plane: screenshot, keys, pointer, pause |
| Monitor socket | `target/roxy/agent-debug/monitor.sock` | Human Monitor Protocol (HMP) |
| GDB stub | `tcp:127.0.0.1:1234` | Attach GDB to the running kernel |
| PID | `target/roxy/agent-debug/qemu.pid` | To stop the VM |

The VM boots normally (APs come up, mlibc loads, `bash-5.3#` appears). It is not paused, so
breakpoints taken *at boot* may be missed; attach GDB early if you need them.

## Reading the screen and the serial console

- Serial output is continuously written to `serial.log` with **no buffering risk** (verified:
  OVMF and kernel output appear within ~1s and roll in promptly). Tail it with:
  ```sh
  tail -n 40 target/roxy/agent-debug/serial.log | cat -v   # cat -v shows ANSI/CR as readable
  ```
  The `cat -v` filter is important: terminal output is full of `^[[...m` escape codes and `^M`
  carriage returns.
- The `serial.log` only carries diagnostics that the kernel tees from the framebuffer terminal.
  The interactive shell's **input is not on serial** — typing goes through the framebuffer. To
  *see the screen*, use `screendump` (see `screenshot.md`).

## Which control plane to use

Prefer **QMP** (`qmp.sock`) for anything scripted/agent-driven: it speaks newline-delimited JSON,
has no readline echo noise, and can run HMP commands too via `human-monitor-command`. Use the raw
HMP socket (`monitor.sock`) only for occasional human-interactive input.

A minimal QMP helper pattern — each connection must first run `qmp_capabilities`, then the
requests, for QEMU to answer (the trailing `\n` after each JSON object is required):

```sh
# qmp: send one JSON request on a fresh connection and print the replies
qmp() { # $1 = JSON request
  local sock=target/roxy/agent-debug/qmp.sock
  timeout 5 bash -c "{ printf '{\"execute\":\"qmp_capabilities\"}\\n'; sleep 0.1; \
        printf '$1\\n'; sleep 0.2; } | nc -U $sock"
}

qmp '{"execute":"query-status"}'   # => {"return":{"status":"running","running":true}}
```

For rapid keyboard/mouse sequences, batch the QMP requests into one stream (one `nc -U` holds
the connection open) rather than opening a new connection per key — see `keyboard.md`.

## Pausing and resuming execution

`cargo xagent-debug` leaves the VM running. To pause/resume for stable inspection or to set
breakpoints before continuing, use QMP directly:

```sh
printf '{"execute":"qmp_capabilities"}\n{"execute":"stop"}\n' | nc -U .../qmp.sock   # pause
printf '{"execute":"qmp_capabilities"}\n{"execute":"cont"}\n' | nc -U .../qmp.sock  # resume
```

`query-status` reports `paused`/`running`. Note QEMU emits `STOP`/`RESUME` events; the `\n`
after `qmp_capabilities` and between requests is required.

## Channel-specific guides

- **Looking at the screen**: [`screenshot.md`](screenshot.md) — `screendump` a PNG, inspect it,
  and read the framebuffer dimensions (needed to convert pointer coordinates; defaults to
  1280x800 on this machine).
- **Keyboard**: [`keyboard.md`](keyboard.md) — inject key events via QMP `input-send-event` or
  HMP `sendkey`; the guest's keyboard goes PS/2 → evdev → `/dev/tty`, so this is the only way to
  type into the framebuffer shell.
- **Mouse**: [`mouse.md`](mouse.md) — move/click the virtual PS/2 mouse with QMP `input-send-event`
  (relative motion) or HMP `mouse_move`/`mouse_button`; translate screen coordinates to relative
  deltas using the framebuffer size from `screenshot.md`.
- **GDB**: [`gdb.md`](gdb.md) — attach to `tcp:1234` with the release kernel ELF for symbol and
  source-level debugging of the *running* kernel, including SMP vCPU selection.

## Stopping the VM

```sh
kill "$(cat target/roxy/agent-debug/qemu.pid)"
```

Use `SIGTERM` first; the stub closes cleanly. If it survives, escalate to `SIGKILL`. Always
confirm the GDB TCP port is free before relaunching (a stale instance holds `tcp:1234`).

## Troubleshooting

- **`tcp:1234` already in use**: an earlier `xagent-debug` (or leftover test QEMU) is still
  running. Kill by pidfile, or `pgrep -af qemu-system`.
- **QEMU died immediately**: check `target/roxy/agent-debug/qemu.log` (QEMU's stderr landing
  spot) and confirm `OVMF_CODE` is set in the dev shell.
- **`qmp.sock` connection stalls**: run every command under `timeout` (usually `timeout 5`);
  a request expecting a reply will otherwise hang the invoking shell.
- **Screenshot shows garbage/absent**: `-display none` still exposes a framebuffer, but if the
  guest hasn't set a mode yet the buffer is small. Wait for boot, or grep `serial.log` for the
  framebuffer init message.