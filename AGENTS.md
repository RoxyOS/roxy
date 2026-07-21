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
- Do not duplicate `STYLE.md`, API rustdoc, code comments, changelogs, temporary plans, TODO lists,
  or source line references in `DESIGN.md`. Design documentation complements local API, safety,
  and invariant comments; it does not replace them.

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

## Plan Mode

- Plans must be implementation-ready and sufficiently detailed that another engineer can execute
  them without rediscovering the intended design.
- Begin by stating the objective, current behavior, desired behavior, scope, explicit non-goals,
  constraints, assumptions, and measurable completion criteria.
- Before proposing implementation steps, inspect the relevant code paths, applicable `AGENTS.md`,
  `STYLE.md`, subsystem `DESIGN.md` files, tests, manifests, and related history when necessary.
- Identify the affected subsystems, files, modules, symbols, public interfaces, ownership
  boundaries, dependencies, and important callers. Do not invent file or symbol names that have
  not been verified in the repository.
- Organize the plan in dependency order. Separate investigation, design decisions, implementation,
  tests, documentation, and final validation when those are distinct phases.
- Each implementation step must explain:
  - the purpose of the change;
  - the specific files, modules, types, functions, or interfaces involved;
  - the intended logic and control or data flow;
  - relevant invariants, ownership, concurrency, locking, interrupt-context, and safety concerns;
  - error handling, unsupported cases, boundary conditions, and compatibility impact; and
  - required tests and the observable result that completes the step.
- For API, ABI, data-layout, configuration, persistence, or dependency changes, describe both
  producers and consumers, migration or compatibility requirements, and all required validation.
- Include the exact validation strategy supported by the repository's current stage: formatting,
  static analysis, builds, unit tests, generated-artifact checks, architecture checks, and any
  necessary manual verification.
- Identify risks, unresolved assumptions, meaningful rejected alternatives, and decisions that
  require user input. First attempt to resolve questions by inspecting the repository.
- State which design and user-facing documentation must be updated. Treat documentation updates as
  part of the implementation rather than optional follow-up work.
- End with a final integration review covering changed files, test coverage, documentation
  consistency, temporary-code removal, and the repository completion criteria.
- Keep plan steps concrete and independently verifiable. Avoid vague steps such as "implement the
  feature", "handle edge cases", or "add tests" without specifying what changes and what behavior
  is verified.
- Detail should describe engineering decisions and observable outcomes, not low-value editor
  operations such as opening files, typing individual lines, or saving files.
- When investigation changes an assumption, revise the plan before continuing so that it remains
  an accurate description of the intended implementation.

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

## mlibc Development Workflow

- Iterate on mlibc in the persistent `distro/sources/mlibc-workdir` tree. Do not update the recipe
  commit or fetch upstream for every local change.
- Use the persistent Jinx and Meson build directories for incremental package builds:
  `cd target/jinx && jinx build mlibc`. Use `jinx rebuild mlibc` only when the incremental build
  state is invalid or configure state must be recreated.
- Rebuild the root filesystem only when userspace artifacts must be tested. Delete
  `target/roxy/rootfs.img`, then run `cargo run -p xtask -- rootfs`; the normal rootfs workflow
  otherwise reuses any structurally valid cached image.
- After refreshing the rootfs, use `cargo run -p xtask -- run` for manual userspace verification.
  Kernel-only changes should reuse the existing rootfs and must not trigger an mlibc rebuild.
- Once local behavior is stable, commit or export the mlibc change to the RoxyOS fork, update the
  recipe `commit` and `version`/`revision`, and perform one clean package/rootfs build before
  treating the change as reproducible. Keep upstream pin changes out of the local iteration loop.
