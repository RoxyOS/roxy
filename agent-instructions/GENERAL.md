# General Repository Instructions

## Instruction Hierarchy

- When changing a subsystem, also look for and follow a subsystem-local style guide such as
  `kernel/syscall/STYLE.md` when one exists.
- Treat `agent-instructions/STYLE.md` as the sole source of truth for coding style; consult it
  instead of duplicating style rules in other instruction files.
- Repository policy is written in English. Always speak to the user in Chinese.

## User Handoff

- After completing any change, explain to the user what changed, why it changed, and the resulting
  behavior. Do not limit the final response to a file list, validation commands, or a statement
  that the task is complete.
- When a change affects design, explain the resulting architecture, responsibility and ownership
  boundaries, and important control or data flow. When architecture is unchanged, state the local
  behavior or implementation contract that changed instead.
- Report compatibility effects, intentional limitations, validation performed, and any remaining
  failures or follow-up work. Calibrate detail to the size of the change, but provide enough context
  for the user to understand and evaluate the result without reading the diff first.

## Git Operations

- Read-only Git commands, including `status`, `log`, `diff`, `show`, and `reflog`, do not require
  user confirmation.
- Before running a Git command that modifies repository, index, branch, reference, remote, or
  worktree state, ask the user for explicit confirmation. This includes `add`, `commit`, `switch`,
  `checkout`, `restore`, `rebase`, `reset`, `stash`, `merge`, `pull`, `push`, `clean`, and commands
  that invoke equivalent mutation through options or subcommands.

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

## Userspace Debugging

- When debugging userspace software, locate and inspect the relevant version of its source code as
  needed. Do not guess at its behavior or attempt to infer it from the binary alone when source is
  available; use disassembly or other binary analysis only when the source is unavailable or the
  investigation specifically requires it.

## Module Layout

- Do not place a `foo.rs` file beside a `foo/` directory. Use `foo/mod.rs` as the module entry
  point and place sibling modules such as `foo/bar.rs` inside that directory.

## Subsystem Design

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
- Do not duplicate `agent-instructions/STYLE.md`, API rustdoc, code comments, changelogs, temporary
  plans, TODO lists, or source line references in `DESIGN.md`. Design documentation complements
  local API, safety, and invariant comments; it does not replace them. Never rely on a maintainer
  having read `DESIGN.md` to understand source structure: the code and its nearby comments must
  explain non-obvious relationships on their own.

## Design and Safety

- Keep unsafe code local. Every unsafe block or unsafe implementation must have a nearby `SAFETY`
  explanation covering the relevant caller obligations and invariants.
- Represent C and userspace ABI records with typed `#[repr(C)]` structs whenever practical. Model
  padding explicitly and initialize every field; do not encode or decode structured ABI data by
  manually indexing raw byte buffers. Convert a typed value to bytes only at the I/O boundary, keep
  that conversion local, and document its layout and lifetime invariants in the `SAFETY` comment.
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
