# Roxy OS Project Overview

Use this document to establish project context before investigating a task. It describes the
stable repository shape and development flow; subsystem `DESIGN.md` files remain authoritative for
their local contracts and implementation decisions.

## What Roxy OS Is

Roxy OS is an x86_64 operating system with a Rust `no_std` kernel and a small Unix-like userspace.
The kernel is split into focused workspace crates and composed into one executable by
`kernel/main`. It boots through Limine under UEFI, mounts an ext4 root filesystem supplied as a
boot module, and starts `/bin/sh` as its initial userspace process.

Userspace is cross-built with Jinx from pinned recipes under `distro`. Its core is Roxy's mlibc
port, Bash, and a deliberately limited BusyBox configuration. The kernel ABI and mlibc sysdeps are
one cross-repository contract: syscall or ABI work may require coordinated changes to both sides.

The current platform is intentionally narrow. The supported architecture backend is x86_64, the
kernel and scheduler are BSP-oriented, the QEMU configuration uses one virtual CPU, and the build
tooling assumes an x86_64 Linux host. Kernel Rust targets `x86_64-unknown-none`; userspace Clang
targets `x86_64-unknown-roxy`.

The current userspace exposes only the Roxy ABI, but the kernel architecture must remain capable
of hosting multiple Unix-like ABI personalities, including Linux-, BSD-, and Solaris-compatible
interfaces. The syscall subsystem is the sole userspace layout boundary. Kernel subsystems below
it exchange ABI-neutral types and must not depend on one personality's record layout, padding,
request numbers, or calling conventions.

## Repository Map

- `kernel/main`: the kernel executable and composition root. Its entry point defines global
  initialization order and selects normal boot or the in-kernel test harness.
- `kernel/<subsystem>`: small `roxy-*` crates for architecture, boot metadata, memory, scheduling,
  processes, syscalls, filesystems, terminals, and related services. `kernel/syscall` contains the
  userspace ABI personalities and translates them into shared kernel types. Read the owning
  `DESIGN.md` before changing one.
- `kernel/test`: the distributed in-kernel test registry. Subsystem tests are linked into a special
  kernel image and run inside QEMU.
- `distro`: the Jinx package graph, Roxy toolchain configuration, source pins, and package patches.
  `distro/recipes/base/recipe` defines the userspace installed into the root filesystem.
- `xtask`: the developer entry point for checks, kernel builds, rootfs creation, ISO creation, and
  QEMU execution.
- `agent-instructions`: repository-wide agent policy. More specific `DESIGN.md` and `STYLE.md`
  files apply within their owning subsystem.
- `ISSUES.md`: known correctness limitations that must not be mistaken for newly introduced bugs.
- `target/roxy` and `target/jinx`: generated images, staging trees, caches, and Jinx build state;
  these are artifacts rather than source.

The root `Cargo.toml` lists every Rust workspace member and makes the dependency graph easy to
trace. Crate names use the `roxy-*` prefix except for the composition binary, `kernel-main`, and the
host-side `xtask` tool.

## Runtime Shape

The durable top-level boot sequence is:

```text
Limine/UEFI → serial and boot metadata → architecture and memory
→ terminal and time → ext4 rootfs → CPU, process, futex, and syscall services
→ interrupts → /bin/sh → scheduler
```

`kernel/main/DESIGN.md` owns the exact initialization contract. The root filesystem image is
embedded in the boot ISO as a Limine module, adapted to a RAM-backed block device, and mounted by
the VFS/ext4 stack. Normal builds prefer the framebuffer terminal and retain serial diagnostics;
kernel-test builds use serial and terminate QEMU through its debug-exit device.

## Development Entry Points

Enter the pinned development environment with `nix develop`. It provides the pinned nightly Rust
toolchain, Jinx, QEMU, OVMF, LLVM tools, and image-building utilities. `OVMF_CODE` is set by this
shell and is required for QEMU runs.

The standard commands are:

- `cargo xcheck`: format check, workspace check and Clippy, release kernel and test-kernel checks,
  plus `git diff --check`. It does not build userspace.
- `cargo xtest`: build the test kernel and run the distributed kernel tests in headless QEMU.
- `cargo xrun`: build the normal kernel and run Roxy OS in QEMU with graphical framebuffer output
  and serial attached to the invoking terminal.
- `cargo xtask image`: create `target/roxy/roxy.iso` without launching QEMU.
- `cargo rootfs`: rebuild the Jinx `base` package staging tree and
  `target/roxy/rootfs.img` from userspace inputs.

Image, run, and test commands reuse an existing structurally valid rootfs image. They do not infer
that a distro or mlibc edit made the cache stale, so rebuild it explicitly after userspace changes.
Kernel-only iteration should reuse the cached rootfs. Package and mlibc work has additional
procedures in `agent-instructions/WORKFLOW.md`.
