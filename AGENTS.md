# Roxy OS Agent Instructions

## Project Info

This is the Roxy OS repository: an x86_64 operating system with a Rust `no_std` kernel and a small
Unix-like userspace. The kernel is split into focused workspace crates and composed into one
executable by `kernel/main`. It boots through Limine under UEFI, mounts an ext4 root filesystem
supplied as a boot module, and starts `/bin/sh` as its initial userspace process.

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

### Repository Map

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
- `AGENTS.md`: repository-wide agent policy. More specific `DESIGN.md` and `STYLE.md`
  files apply within their owning subsystem.
- `ISSUES.md`: known correctness limitations that must not be mistaken for newly introduced bugs.
- `target/roxy` and `target/jinx`: generated images, staging trees, caches, and Jinx build state;
  these are artifacts rather than source.

The root `Cargo.toml` lists every Rust workspace member and makes the dependency graph easy to
trace. Crate names use the `roxy-*` prefix except for the composition binary, `kernel-main`, and the
host-side `xtask` tool.

### Runtime Shape

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

### Development Entry Points

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
procedures in the jinx and mlibc skills (`.pi/skills/jinx/`, `.pi/skills/mlibc/`).

## Rules

### Instruction Hierarchy

- When changing a subsystem, also look for and follow a subsystem-local style guide such as
  `kernel/syscall/STYLE.md` when one exists.
- Repository policy is written in English. Always speak to the user in Chinese.

### Question Context

- Questions inside this repository about how software behaves or is made to work — Xorg/X11
  input, drivers, terminals, or any userspace or kernel feature — default to the Roxy OS
  implementation under development here, not to the user's NixOS host.
- Only treat a question as host-NixOS configuration when the user explicitly mentions the
  host OS.
- When the intended target is genuinely ambiguous, ask one clarifying question before
  answering rather than guessing.

### User Handoff

- After completing any change, explain to the user what changed, why it changed, and the resulting
  behavior. Do not limit the final response to a file list, validation commands, or a statement
  that the task is complete.
- When a change affects design, explain the resulting architecture, responsibility and ownership
  boundaries, and important control or data flow. When architecture is unchanged, state the local
  behavior or implementation contract that changed instead.
- Report compatibility effects, intentional limitations, validation performed, and any remaining
  failures or follow-up work. Calibrate detail to the size of the change, but provide enough context
  for the user to understand and evaluate the result without reading the diff first.

### Git Operations

- Read-only Git commands, including `status`, `log`, `diff`, `show`, and `reflog`, do not require
  user confirmation.
- Before running a Git command that modifies repository, index, branch, reference, remote, or
  worktree state, ask the user for explicit confirmation. This includes `add`, `commit`, `switch`,
  `checkout`, `restore`, `rebase`, `reset`, `stash`, `merge`, `pull`, `push`, `clean`, and commands
  that invoke equivalent mutation through options or subcommands.
- The mlibc publication and recipe-integration workflow in `.pi/skills/mlibc/SKILL.md` is an
  explicit exception. When a task changes Roxy's mlibc sysdeps, follow that workflow through its
  ordinary commits, pushes, and recipe update without requesting additional confirmation.

### Architecture and Dependencies

- Do not copy kernel code from Seele. Seele may only be consulted as a behavioral reference or an
  architectural failure case.
- Do not create placeholder crates, speculative abstractions, compatibility shims, silent stubs,
  or test-name special cases.
- Any implementation that approximates, degrades, substitutes, or stubs behavior because a
  capability is not yet implemented must carry an in-place `TODO(<missing-capability>)` or
  `FIXME` comment naming the gap and the intended final behavior. A plain doc comment is not
  enough: it reads as intended design rather than as debt. Substantial gaps are also recorded in
  `ISSUES.md` in the same style as the existing ext4 `FIXME` entries.

### Userspace Debugging

- When debugging userspace software, locate and inspect the relevant version of its source code as
  needed. Do not guess at its behavior or attempt to infer it from the binary alone when source is
  available; use disassembly or other binary analysis only when the source is unavailable or the
  investigation specifically requires it.

### Subsystem Design

- Before changing a subsystem, find and read every applicable `DESIGN.md`, starting at the
  subsystem root and following any more specific document in the directory being changed.
- After changing a subsystem, review its `DESIGN.md` and update it in the same change so that it
  still describes the implemented design. Do not leave ownership rules, cross-subsystem contracts,
  or architectural rationale only in source comments.
- Create a subsystem `DESIGN.md` when a design-level change has no applicable design document.
  Local implementation changes that do not affect the documented design do not require a new
  document.
- A subsystem design document should cover the parts that apply to that subsystem:
  - purpose, scope, and explicit non-goals;
  - responsibilities, resource ownership, and dependency boundaries;
  - invariants and important lifecycle, control-flow, or data-flow sequences;
  - extension points and hooks, including who registers or calls them, when they run, their
    locking or interrupt context, and what they must guarantee;
  - concurrency and safety assumptions, failure behavior, unsupported cases, and current
    limitations; and
  - important rejected alternatives when their tradeoffs are likely to be reconsidered.
- Write design documents in English for long-term maintainers. Explain why the design exists and
  state durable contracts rather than walking through functions or restating the current code.
  Use compact diagrams or tables only when they clarify a relationship or sequence.
- Do not duplicate API rustdoc, code comments, changelogs, temporary plans, TODO lists, or
  source line references in `DESIGN.md`. Design documentation complements
  local API, safety, and invariant comments; it does not replace them. Never rely on a maintainer
  having read `DESIGN.md` to understand source structure: the code and its nearby comments must
  explain non-obvious relationships on their own.

### Design and Safety

- Userspace ABI layouts belong exclusively to the syscall subsystem. Define userspace-facing
  `#[repr(C)]` records, explicit ABI padding, size/offset assertions, request numbers, and raw
  pointer interpretation only under `kernel/syscall`; never expose those records through a shared
  kernel API or reproduce them in process, FD, TTY, filesystem, or other domain subsystems.
- Represent each userspace ABI record in the syscall subsystem with a typed `#[repr(C)]` struct.
  Model padding explicitly and initialize every field; do not encode or decode structured ABI data
  by manually indexing raw byte buffers. Decode immediately into ABI-neutral kernel types before
  dispatch and encode subsystem results only when returning through the selected ABI personality.
  Keep byte conversion local and document its layout and lifetime invariants in the `SAFETY`
  comment.
- A `#[repr(C)]` type outside `kernel/syscall` is permitted only for a non-userspace contract such
  as an architecture context, hardware-defined record, or internal foreign-function boundary. Its
  owning design and nearby source must identify that contract so it cannot be mistaken for a
  userspace ABI layout.
- **IMPORTANT**: **Never** reject, terminate, block indefinitely, silently degrade, or return any
  error for a userspace request because kernel functionality is missing or incomplete without
  first emitting an unconditional serial `UNSUPPORTED` diagnostic naming the syscall or operation,
  unsupported mode or argument, PID/TID, and returned errno. All such paths, including unknown
  syscalls and partially implemented interfaces, must use the repository's centralized
  unsupported-operation helper. Direct returns of `ENOSYS`, `ENOTSUP`, or `EOPNOTSUPP` are
  forbidden and must be rejected by tests or static checks.
