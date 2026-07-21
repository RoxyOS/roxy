# Code Review Instructions

## Purpose

Review changes for correctness, safety, maintainability, architectural consistency, and
repository-policy compliance. A successful build or a clean formatter run is not sufficient
evidence that a change is ready.

Review the proposed change rather than performing an unrelated repository-wide audit, but inspect
enough surrounding code, callers, tests, manifests, and history to understand its effects. Report
pre-existing problems only when the change relies on them, worsens them, or makes them newly
relevant.

## Sources of truth

Before reviewing source code:

1. Read `AGENTS.md`, `STYLE.md`, and the applicable parts of `WORKFLOW.md`.
2. Find every `AGENTS.md`, `STYLE.md`, and `DESIGN.md` whose scope covers a changed file. More
   specific documents supplement or override broader guidance as described by `AGENTS.md`.
3. Read affected manifests, public interfaces, tests, and important callers. For cross-subsystem
   changes, inspect both sides of every ownership or API boundary.
4. Use repository history when the intent or an invariant cannot be established from the current
   tree. Do not treat old code as proof that a pattern is correct today.

`STYLE.md` is the sole source of truth for coding style. Apply it directly; do not replace it with
personal preferences or generic Rust conventions.

## Establish the review scope

- Identify the intended base and include staged, unstaged, and relevant untracked files. Do not
  silently review only one part of a mixed working tree.
- Summarize the behavior that changes, the affected subsystems, and the observable completion
  criteria before judging the implementation.
- Trace changed public APIs, data layouts, configuration, persistent data, generated artifacts,
  recipes, and ABI definitions to all producers and consumers.
- Separate defects introduced by the change from optional improvements. A preference that has no
  correctness, maintenance, design, or policy impact is not a finding.

## Correctness and behavior

Verify the change against its stated behavior and realistic failure modes:

- Follow control and data flow through success, error, cleanup, retry, cancellation, and partial
  initialization paths. Look for stale state, leaks, double cleanup, and misleading success.
- Check boundary values, integer conversion and overflow, empty and maximum-sized inputs, invalid
  enum or flag combinations, alignment, address ranges, and cross-page operations where relevant.
- Check that errors preserve useful context, use the correct errno or error type, and do not hide
  missing behavior behind a fallback, panic, ignored result, or silent stub.
- Check lifecycle and ownership transitions, including `Drop`, clone/share behavior, registration
  order, initialization order, and whether resources outlive the state that validates them.
- Check concurrency assumptions, lock ordering, interrupt context, reentrancy, atomicity, races,
  wakeup/lost-wakeup behavior, and all state observed outside a lock.
- Confirm that configuration gates and architecture-specific paths select one coherent behavior
  and that unsupported targets fail explicitly rather than compiling an incomplete path.
- Reject test-name special cases, temporary instrumentation, speculative compatibility paths, and
  workarounds that leave the root cause intact.

## Kernel, unsafe code, and userspace boundaries

- Treat all userspace-controlled values as hostile. Confirm that raw pointers, lengths, handles,
  flags, paths, and structures are validated and converted at the subsystem boundary before use.
- For every unsafe block or unsafe implementation, require a nearby `SAFETY` explanation that
  states the relevant invariants and caller obligations. Independently verify that the code
  establishes those invariants; a comment alone is not evidence of soundness.
- Verify memory lifetime, aliasing, initialization, provenance, pinning, mapping, and
  synchronization assumptions around unsafe code and architecture interfaces.
- For syscall changes, apply `kernel/syscall/STYLE.md`, preserve the parse/validate/delegate flow,
  and keep policy in the owning subsystem instead of duplicating it at the ABI boundary.
- Any userspace request rejected because functionality or a mode is missing must use the
  centralized unsupported-operation path. Confirm that its unconditional serial diagnostic names
  the operation, unsupported mode or argument, PID/TID, and returned errno. Direct `ENOSYS`,
  `ENOTSUP`, or `EOPNOTSUPP` returns are release-blocking findings.

## Design and maintainability

Judge whether the implementation remains understandable and safely extensible after the immediate
change:

- Confirm that responsibilities remain in the owning subsystem and that dependencies point in the
  intended direction. Third-party or backend-specific types must not leak through unrelated public
  APIs.
- Look for duplicated policy, scattered validation, temporal coupling, hidden global state,
  boolean mode combinations, overly broad visibility, and abstractions whose invariants cannot be
  stated clearly.
- Require names, types, enums, newtypes, RAII guards, and state transitions to make important units,
  lifetimes, ownership, and invalid states visible as directed by `STYLE.md`.
- Check that public interfaces are the smallest interface needed by current callers and that a
  refactor migrates every caller and removes the obsolete path unless an explicit migration plan
  exists.
- Distinguish genuine reuse from speculative generalization. New traits, macros, helpers,
  compatibility shims, and placeholder modules need a concrete current use and a clear owner.
- Verify that comments explain rationale or invariants rather than compensating for unclear code.
  Durable ownership rules and cross-subsystem contracts belong in the applicable `DESIGN.md`.

## Files and module boundaries

Review file organization as part of maintainability, not as a cosmetic concern:

- Apply the source-file and function limits from `STYLE.md`, counting non-blank lines where needed.
  A file approaching a hard limit must still have a coherent reason to remain intact.
- Prefer modules split by responsibility, behavior, or lifecycle phase. Reject arbitrary line-count
  splits, large files separated only by section comments, and catch-all modules with unrelated
  responsibilities.
- When a module needs child files, require `foo/mod.rs` with siblings inside `foo/`. Do not allow a
  `foo.rs` file beside a `foo/` directory.
- Check that moving or splitting code preserves visibility, ownership, safety explanations, tests,
  and a discoverable public entry point. A split that only moves complexity without clarifying a
  boundary is not an improvement.
- Confirm utility code is genuinely shared and owned by the narrowest appropriate subsystem. Apply
  the repository naming rule for general utility modules from `STYLE.md`.

## Dependencies, manifests, and external code

- For newly implemented general-purpose functionality, verify that the change considered a
  suitable existing crate or upstream implementation. Require the dependency choice to be
  documented, including target and `no_std` support, maintenance, soundness, license, and API fit
  when those factors apply.
- Check that dependencies are declared in the root workspace table and inherited by members, with
  versions, features, grouping, and ordering conforming to `STYLE.md`.
- Review feature selection and default features for unwanted allocation, standard-library,
  platform, or transitive behavior. Ensure a lockfile change is explained by its manifest change.
- Do not accept kernel code copied from Seele, placeholder crates, or local reimplementations that
  create an avoidable maintenance burden.
- For distro changes, follow `WORKFLOW.md`: verify recipe ownership, patch placement and ordering,
  revision updates, clean-tree applicability, generated-artifact exclusions, and affected
  dependent packages.

## ABI and compatibility

- For API, ABI, layout, syscall, or generated-definition changes, identify every producer and
  consumer and verify that all of them move together.
- Check syscall numbers, Rust and C declarations, argument and return representations, errno
  behavior, alignment, symbol exposure, registry tests, mlibc sysdeps, and generated artifacts as
  applicable.
- Require an explicit compatibility or migration decision. Accidental compatibility breaks and
  old/new implementations left in parallel are findings even when the local crate builds.
- Require the typed ABI generation, Rust/C layout checks, and userspace static-library symbol checks
  mandated by `AGENTS.md` for every ABI change.

## Tests, validation, and documentation

- Require tests to change with behavior. Tests should cover externally observable behavior,
  important boundaries, failure paths, and regressions rather than merely reproduce implementation
  details.
- Confirm that tests would fail for the defect they claim to prevent. Watch for assertions that are
  too broad, ignored errors, nondeterministic timing assumptions, leaked global state, and cases
  that pass only because of execution order.
- Select the strongest validation supported by the repository's active stage and the affected
  subsystem. Do not request unavailable CI or test layers, and do not accept claims for commands
  that were not run.
- At minimum, inspect formatting/whitespace status, relevant builds and static checks, the
  distributed kernel unit harness where applicable, generated artifacts, architecture contracts,
  and the package/rootfs workflow required by the change.
- Review all applicable `DESIGN.md` files after reading the code. Report stale contracts,
  undocumented design-level changes, and design documents that merely restate implementation.
  Require a new subsystem `DESIGN.md` only when the change introduces design-level behavior with
  no applicable document.
- Check user-facing and workflow documentation for changed commands, configuration, limitations,
  or operational behavior. Remove temporary plans, debug instructions, and obsolete TODOs.
