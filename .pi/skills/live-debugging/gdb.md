# GDB Attach to the Running Roxy Kernel

`cargo xagent-debug` starts a GDB stub on `tcp:127.0.0.1:1234`. Connect GDB to debug the *live*
kernel — registers, breakpoints, single-step, memory.

## Which ELF

Connect the ELF matching the `--profile` you launched — GDB must symbolize the exact running
build:

- `--profile dev` → `target/x86_64-unknown-none/debug/kernel-main` (DWARF, source lines work).
- `--profile release` → `target/x86_64-unknown-none/release/kernel-main` (symbols only, mangled
  names, no source lines).

```sh
gdb -q target/x86_64-unknown-none/debug/kernel-main
(gdb) target remote 127.0.0.1:1234
```

## Attaching

- `target remote` **pauses** the VM immediately.
- The VM boots on its own; pause it right after start (QMP `stop`) to catch early boot.

## SMP

16 vCPUs. `info threads` / `thread <id>` to switch. Only the selected thread steps; prefer
`thread apply all stop`, or `set scheduler-locking on` while stepping.

## Finishing

Do **not** leave the VM paused: send `continue` before `detach`, then stop via the pidfile.
If breakpoints don't hit, the code already ran past them (attached mid-boot); re-pause, set
breakpoints, continue — or restart and attach earlier.
