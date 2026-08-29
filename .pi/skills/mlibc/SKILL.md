---
name: mlibc
description: Use when working on mlibc
---

# mlibc — Roxy OS's C library

> **Read the jinx skill first.** Distro build mechanics — workdir edits, `jinx regen`, patches,
> recipes, and the `-clean`/`-workdir`/`patches/` layout — are covered by the jinx skill
> (`.pi/skills/jinx/SKILL.md`, `patch.md`). This skill only covers mlibc-specific development
> and publishing.

## What mlibc is

mlibc is the C library every Roxy OS userspace package links against. Roxy OS maintains its own
fork at `github.com/RoxyOS/mlibc` (branch `master`).

The fork's purpose is the **roxy sysdeps**: the OS-specific layer that implements mlibc's sysdep
interface on top of Roxy OS kernel syscalls. This is what you'll most often modify.

## Repository layout (fork-relevant parts)

```
mlibc/
├── sysdeps/roxy/                  ← the Roxy OS sysdeps (this is what you edit)
│   ├── meson.build                ← BUILD REGISTRY: which sources compile, which headers install
│   ├── <domain>.cpp               ← sysdep implementations, one file per domain
│   │                                (signal, socket, filesystem, ioctl, ...)
│   ├── arch/<arch>/               ← per-arch ABI: syscall.cpp (roxy_syscall0..6),
│   │                                restorer.S (signal restorer)
│   ├── crt-<arch>/                ← startup objects (crt1/crti/crtn.S)
│   └── include/
│       ├── roxy/syscall.h         ← ABI CONTRACT: syscall numbers + result structs
│       ├── abi-bits/              ← installed ABI headers (stat.h, errno.h, ...)
│       ├── mlibc/sysdeps.hpp      ← sysdep interface overrides
│       └── sys/
└── options/…                      ← the rest of upstream mlibc (generic + linux options)
```

## How the sysdeps work (mental model)

mlibc core calls a **sysdep interface** (`mlibc::sysdep<FutexWait, ...>` etc.). The roxy sysdeps
implement those operations in `namespace mlibc` as `Sysdeps<Operation>::operator()(...)`.

The chain for a typical syscall-backed operation:

1. A domain `.cpp` in `sysdeps/roxy/` implements e.g. `Sysdeps<Write>::operator()` — calls
   `roxy_syscall3(ROXY_SYS_WRITE, ...)`.
2. `arch/x86_64/syscall.cpp` provides `roxy_syscall0..6` — the actual `syscall` instruction
   (number in `rax`, args `rdi/rsi/rdx/r10/r8/r9`, result in `rax`).
3. `include/roxy/syscall.h` defines the **syscall numbers** (`ROXY_SYS_*`) and the ABI result
   structs (`roxy_stat_result`, `roxy_clock_result`, `roxy_dirent`) with static_asserts pinning
   their layout.

Error convention: negative result = `-errno`; helpers `syscall_error()` / `syscall_result()`
convert.

## The syscall-number ABI contract

The syscall numbers are a **duplicated contract** between two repos, kept in sync by hand:

- **Kernel side**: `kernel/syscall/src/numbers.rs` in the roxy repo (`enum SyscallNumber`).
- **Libc side**: `#define ROXY_SYS_*` in `sysdeps/roxy/include/roxy/syscall.h`.

Adding a syscall means adding the enum variant in `numbers.rs` AND the matching `#define` — same
number, same order. A mismatch silently breaks that syscall.

## Modifying the roxy sysdeps

### Adding a new syscall-backed sysdep

1. **Kernel side first**: add the variant to `enum SyscallNumber` in `kernel/syscall/src/numbers.rs`
   and implement the handler under `kernel/syscall/src/syscalls/` (see the dispatch/registry
   modules).
2. **Mirror the number**: add `#define ROXY_SYS_XXX <n>` in `include/roxy/syscall.h` — must match
   the enum value exactly.
3. **Implement the operation**: add `Sysdeps<Operation>::operator()` in `sysdeps.cpp` (or the
   matching per-domain file), calling `roxy_syscallN` with the correct arg count.
4. **Register the tag**: add the operation's tag to `struct RoxySysdepTags` in
   `sysdeps/roxy/include/mlibc/sysdeps.hpp` — a compiled but undiscoverable sysdep is incomplete.
5. **Register the file**: if you put code in a new `.cpp`, add it to `libc_sources` (or
   `rtld_sources` for loader bits) in `sysdeps/roxy/meson.build`.
6. **Result structs**: if the syscall returns a struct, define it in `syscall.h` **with
   static_asserts** on size/alignment/offsets — the kernel ABI depends on this layout.
7. **ABI headers**: if a new public header is needed, add it to the `install_headers` list in
   `meson.build` (headers live under `include/abi-bits/` or `include/sys/`).

### Making changes

The mlibc recipe pins a commit and has `clean_workdirs=no` — the local clone lives at
`distro/sources/mlibc` (workdir: `distro/sources/mlibc-workdir`, the tree you edit).

1. Edit the workdir (`distro/sources/mlibc-workdir`).
2. To test locally: run `jinx regen mlibc`, then `jinx build mlibc`. Re-run `jinx regen` after
   each further workdir edit; the regenerated patch is what actually reaches the build. Use
   `jinx rebuild mlibc` (fresh `configure()`) only when incremental state is invalid or the
   Meson configuration must be recreated — e.g. after changing the recipe pin or `meson.build`.
3. When validated, commit and push to the fork, then update the recipe: bump `commit` to the new
   SHA and `version` per the convention below (see Publishing for the exact rules), and remove
   the temporary `jinx-working-patch.patch`.
4. mlibc is **dynamically linked** (`libc.so`, `ld.so`, ...) — consumers pick up the new libc at
   runtime, so no `revbump` of dependents is needed.

### Publishing a commit and updating the recipe pin

1. Commit in `distro/sources/mlibc-workdir` as one cohesive change per commit, subject form
   `roxy: <imperative summary>`. Do not amend/squash/rewrite fork history; never force-push the
   canonical fork.
2. Push to the canonical `RoxyOS/mlibc` fork; verify the exact commit SHA is reachable before
   touching the recipe.
3. Update `distro/recipes/mlibc/recipe` only after the commit is published: pin the immutable
   SHA. Version convention: `0.0.0.YYYYMMDD` for the first update on a date; append `.1`, `.2`,
   ... for further updates the same day. Keep `revision=1`.
4. One clean `jinx rebuild mlibc` after the pin change, then refresh the rootfs if installed
   behavior or the kernel ABI changed. No `revbump` of dependents — mlibc is dynamically linked
   (`libc.so`/`ld.so`), consumers pick up the new libc at runtime.

## Gotchas

- **Syscall ABI must match the kernel exactly**: numbers, arg order, and result struct layouts
  (that's why `syscall.h` has static_asserts — don't drop them).
- mlibc build needs network at source-prep (`meson subprojects download` for `freestnd-c-hdrs`,
  `frigg`, ...) — a sandboxed env fails there.
- Every userspace package depends on mlibc, but because it is **dynamically linked**, updating
  it does NOT require rebuilding dependents (no `revbump`) — consumers load the new `libc.so` at
  runtime. (`revbump` matters in a static-link world; here it's unnecessary.)
