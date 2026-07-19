# Repository Guidelines

## Source of Truth

- Read `STYLE.md` before changing source code. It defines repository-wide coding conventions.
- When changing a subsystem, also look for and follow a subsystem-local style guide such as
  `kernel/syscall/STYLE.md` when one exists.
- Treat `STYLE.md` as the sole source of truth for coding style; consult it instead of duplicating
  style rules in this file.
- Repository policy is written in English. Speak to the user in the language they use.

## Architecture and Dependencies

- Before implementing functionality, search crates.io, lib.rs, and relevant upstream projects for
  an existing crate.
- Prefer declaring dependencies in the root `[workspace.dependencies]` table and inheriting them
  with `dependency.workspace = true` from member crates.
- Keep manifest version requirements broad when compatible releases are acceptable, for example
  `clap = "4"`; rely on `Cargo.lock` for reproducible exact resolution.
- Document why dependencies were selected.
- Keep third-party types inside the owning subsystem or adapter. Do not expose them through an
  unrelated subsystem's public API.
- Do not copy kernel code from Seele. Seele may only be consulted as a behavioral reference or an
  architectural failure case.
- Do not create placeholder crates, speculative abstractions, compatibility shims, silent stubs,
  or test-name special cases.

## Design and Safety

- Keep unsafe code local. Every unsafe block or unsafe implementation must have a nearby `SAFETY`
  explanation covering the relevant caller obligations and invariants.
- **IMPORTANT**: **Never** reject, terminate, block indefinitely, silently degrade, or return any
  error for a userspace request because kernel functionality is missing or incomplete without
  first emitting an unconditional serial `UNSUPPORTED` diagnostic naming the syscall or operation,
  unsupported mode or argument, PID/TID, and returned errno. All such paths, including unknown
  syscalls and partially implemented interfaces, must use the repository's centralized
  unsupported-operation helper. Direct returns of `ENOSYS`, `ENOTSUP`, or `EOPNOTSUPP` are
  forbidden and must be rejected by tests or static checks.
- Update tests together with behavior changes.
- Remove temporary instrumentation, debugging paths, and experimental workarounds before
  completing a change.

## Workflow

- Use `apply_patch` for manual file edits.
- Preserve user changes and avoid destructive Git commands.
- Do not commit unless the user explicitly requests a commit.
- When commits are requested, keep one logical change per commit.
- Run the strongest checks supported by the active stage. Never claim that a check ran when its
  toolchain or implementation does not exist.

Stage-aware validation:

- Stage 0: local Cargo metadata and basic repository checks.
- Stage 1: local formatting, Clippy, host tests, and Nix/tooling checks.
- Stage 2: local formatting, Clippy, and host checks for the architecture contract.
- Typed ABI generation, Rust/C layout checks, and userspace static-library symbol checks apply to
  every ABI change, including the current exit syscall.
- The repository currently has only a distributed kernel unit harness. It has no CI or other test
  layers; add those only when their stage is explicitly resumed by the project owner.
