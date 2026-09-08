# GDB Attach to the Running Roxy Kernel

`cargo xagent-debug` starts QEMU with a GDB stub on `tcp:127.0.0.1:1234`. Attach GDB to debug the
*live, running* kernel — inspect registers, set breakpoints, single-step, and read memory while
the guest executes. This is complementary to the screenshot/keyboard/mouse channels above.

## The debug symbols binary

GDB needs an ELF with symbols to resolve `break`/`p &symbol`. Two options, matching the
`--profile` you pass to `cargo xagent-debug`:

- **`--profile dev` → debug ELF (source lines)**: `cargo xagent-debug --profile dev` builds
  `target/x86_64-unknown-none/debug/kernel-main` and embeds it in the ISO, so the running kernel
  *is* the debug ELF. Point GDB straight at it for full source-level debugging:
  ```sh
  gdb -q target/x86_64-unknown-none/debug/kernel-main
  (gdb) target remote 127.0.0.1:1234
  ```
  This ELF carries DWARF (`readelf -S` shows `debug_info`/`debug_line`) and unoptimized code, so
  single-stepping and `p` on Rust variables work.
- **`--profile release` → release ELF, symbol-only**: the optimized kernel ships a symbol table
  but **no DWARF** (verified via `readelf`), so with it you get mangled function names and
  addresses but not source lines. Point GDB at:
  ```sh
  gdb -q target/x86_64-unknown-none/release/kernel-main
  (gdb) target remote 127.0.0.1:1234
  ```
  Function breakpoints/backtraces work on the mangled names; `$main`, `$entry` are legible.

> Always connect GDB to the ELF whose profile you launched — the running kernel must be the same
> build GDB symbolizes, or addresses/line tables won't line up.

## Attaching and the initial state

- Connecting via `target remote` **pauses** the VM immediately (QEMU's stub stops it on attach).
- The VM was booting when `xagent-debug` started; if you attached late, early boot is behind you.
  To catch boot itself, pause the VM right after starting `cargo xagent-debug` (QMP `stop`, see
  `SKILL.md`) before attaching GDB.
- GDB stub under `-display none` on the release image works (protocol exchange verified: register
  reads return real AP/init values).

## Working with SMP

The machine is `-smp 16`, so there are 16 vCPUs. GDB's remote stub exposes them; switch with:

```
(gdb) info threads
(gdb) thread <id>
```

Most kernel debugging here needs the current CPU. When you single-step on a multi-CPU target,
only the selected thread steps; others keep running, so prefer `thread apply all stop` then pick a
CPU, or use `set scheduler-locking on` while stepping the thread you care about.

## Useful commands

```
info registers                    # current vCPU state
bt                                # backtrace of the current thread
x/8gx 0xffffffff80000000          # read memory (physical/higher-half kernel VA)
p &some_kernel_global             # address of a kernel symbol
break roxy::module::function      # symbol breakpoint (must have debug symbols to resolve)
continue / stepi / nexti
```

Because much kernel state is in Rust `no_std` types, `x` (memory) and register inspection are
more reliable than `p` on arbitrary types without a full debug-info side table.

## Verifying the debugger is live

- `info registers` returns non-zero values (entry/VMM state differs from a fully booted CPU).
- `monitor info cpus` (via the HMP socket) shows all 16 vCPUs by thread id, which cross-checks
  the GDB thread list.
- The guest is genuinely paused: type `cont`, then a screenshot (`screenshot.md`) or `serial.log`
  resumes changing.

## Leaving the guest running

Do **not** leave the VM paused when you are done — send `continue` before detaching, or the guest
freezes forever. Disconnect with `detach` after continuing, then stop the VM via its pidfile
(`kill "$(cat target/roxy/agent-debug/qemu.pid)"`).

## Notes

- Keep the GDB stub port free: a leftover `xagent-debug` holds `tcp:1234`, so kill stray QEMU
  before relaunching.
- If breakpoints don't hit and you attached mid-boot, the code already ran past them; re-pause,
  set breakpoints, and continue, or restart and attach earlier.