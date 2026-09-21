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

## What the fork may change

Only Roxy-owned code may be modified:

- `sysdeps/roxy/**` — the sysdeps, their `meson.build` registry, `arch/`, `crt-*`, and the
  `include/roxy/` and `include/sys/` headers they install;
- `abis/roxy/**` — the Roxy ABI headers installed as `abi-bits/*` (the entries under
  `sysdeps/roxy/include/abi-bits/` are symlinks into this directory).

Everything else is upstream mlibc and stays untouched: `options/**` — generic implementations,
their headers, and their Meson build files — the other `abis/*` platforms, `options/internal/**`,
and the top-level build files. Editing those turns every upstream merge into a conflict, which is
the cost this fork exists to avoid.

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
├── abis/roxy/                     ← the Roxy ABI headers, symlinked from abi-bits/ below
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

Error convention: the kernel returns the value in `rax` and the error code in `r10`, with `0` in
`r10` meaning success, so a caller must check it before using the value. `roxy_syscall0..6` return a
`roxy_syscall_result` holding both, and the helpers `syscall_error()` / `syscall_result()` convert
that pair into the errno/out-parameter shape mlibc's sysdeps use.

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

### Default mlibc delivery

When a task changes mlibc, the default completed result is:

1. modify the mlibc workdir;
2. commit the change to the RoxyOS/mlibc repository;
3. update `distro/recipes/mlibc/recipe` to the new commit and version;
4. remove any temporary `jinx-working-patch.patch`.

Do not stop at a generated working patch unless the user explicitly asks for a patch-only,
local-only, or unpublished change.


Each mlibc recipe publication must contain exactly one new commit after the previous recipe pin.
Local implementation commits may be split while developing, but they must be squashed before
publication. The published commit's parent must be exactly the commit currently pinned by
`distro/recipes/mlibc/recipe`.

1. Read the previous mlibc pin from `distro/recipes/mlibc/recipe` before changing the recipe.
2. Implement and validate all related Roxy mlibc changes in `distro/sources/mlibc-workdir`.
3. Squash the complete publication into one commit with subject form
   `roxy: <imperative summary>`. Do not rewrite the previous recipe pin or any unrelated
   publication. The new commit must satisfy:

   ```sh
   test "$(git rev-parse <new-sha>^)" = "<previous-pin>"
   test "$(git rev-list --count <previous-pin>..<new-sha>)" -eq 1
   ```

4. Push the squashed commit to the canonical `RoxyOS/mlibc` fork. The workdir is usually on a
   detached HEAD, where `git push origin master` pushes the local `master` — still the previous
   commit — and prints `Everything up-to-date`, a silent no-op. Read the branch first:

   ```sh
   git checkout master                       # or: git switch master
   git reset --hard <new-sha>
   git push --force-with-lease origin master
   git rev-parse HEAD origin/master
   ```

   Force-push is permitted here because the canonical branch is intentionally maintained as one
   publication commit per recipe pin. `--force-with-lease` protects against overwriting a remote
   update that was not observed locally. Verify that both SHAs equal the new commit before touching
   the recipe. `Everything up-to-date` is not evidence that the commit went out.
5. Update `distro/recipes/mlibc/recipe` only after the squashed commit is published: pin the
   immutable SHA and update `version` for the publication date. The version must represent this
   new upstream publication, not remain at the previous recipe version.

   Use the following convention:
   - The first mlibc publication on a date uses `0.0.0.YYYYMMDD`.
   - Additional publications on the same date append `.1`, `.2`, and so on.
   - Determine the next suffix from the versions already present in repository history or the
     current recipe state.
   - Keep `revision=1`.

   Before finishing, verify that the recipe diff changes both `version` and `commit`, while
   leaving `revision=1`. A commit-only update is incomplete. Also verify that the new recipe pin
   is exactly one commit after the old pin:

   ```sh
   test "$(git rev-parse <new-pin>^)" = "<previous-pin>"
   test "$(git rev-list --count <previous-pin>..<new-pin>)" -eq 1
   ```
6. One clean `jinx rebuild mlibc` after the version and pin change, then refresh the rootfs if
   installed behavior or the kernel ABI changed. No `revbump` of dependents — mlibc is dynamically linked
   (`libc.so`/`ld.so`), consumers pick up the new libc at runtime.
7. ABI changes validate the kernel and mlibc contracts together: syscall numbers must match
   `kernel/syscall/src/numbers.rs`, and result-struct layouts must match the kernel's generated
   checks.

## Gotchas

- **A detached workdir turns the push into a no-op**: from a detached HEAD, `git push origin
  master` pushes the local `master`, which still points at the previous commit, and reports
  `Everything up-to-date` — indistinguishable from a successful push unless you compare SHAs.
  Check out `master`, reset it to the squashed publication commit, push with
  `git push --force-with-lease origin master`, and confirm `git rev-parse HEAD` equals
  `git rev-parse origin/master` before touching the recipe.
- **Syscall ABI must match the kernel exactly**: numbers, arg order, and result struct layouts
  (that's why `syscall.h` has static_asserts — don't drop them).
- mlibc build needs network at source-prep (`meson subprojects download` for `freestnd-c-hdrs`,
  `frigg`, ...) — a sandboxed env fails there.
- Every userspace package depends on mlibc, but because it is **dynamically linked**, updating
  it does NOT require rebuilding dependents (no `revbump`) — consumers load the new `libc.so` at
  runtime. (`revbump` matters in a static-link world; here it's unnecessary.)
