# Roxy OS Coding Style

This file defines the coding conventions for repository-owned source code. Hard limits are
mandatory unless a listed exemption applies. All other rules are defaults: depart from them only
when doing so makes the code demonstrably clearer or preserves a stronger local convention.

## Core Principles

- Make intent visible through names, types, ownership, module boundaries, and control flow.
- Prefer direct code and explicit invariants over clever compression or implicit conventions.
- Solve problems structurally: restore invariants, centralize shared behavior, and remove the
  source of duplication instead of layering local workarounds.
- Keep abstractions proportional to current needs. Do not introduce placeholders, speculative
  abstractions, compatibility shims, or generic utilities without a concrete caller.

## Hard Limits

- Keep a source file at or below 150 non-blank lines whenever practical. A source file must not
  exceed 250 non-blank lines.
- Keep a function at or below 30 lines. Extract named operations when it begins to mix validation,
  state construction, side effects, and policy decisions.
- A tuple must contain no more than three elements. Use a dedicated struct with named fields when
  parameters, return values, stored state, or intermediate data require more.
- Generated files and declarative data tables are exempt from the line limits only when splitting
  them would make the result harder to understand.

## Code Organization

- Prefer small modules with narrow responsibilities over large files divided by comments.
- Name general-purpose utility modules `utils.rs`, not `helper.rs` or `helpers.rs`.
- Organize files by behavior or lifecycle phase rather than placing every operation for one type
  in the same file. Spread its `impl` blocks across focused modules when appropriate.
- Write multi-stage pipelines as a short, linear sequence of named operations. The top-level
  function should read like an execution plan while phase-specific details remain in their owning
  modules.
- Represent meaningful lifecycle phases with an explicit state enum. Validate the expected state
  at each phase boundary, preferably with `let ... else`, instead of relying on call order alone.
- Preserve subsystem boundaries. Keep third-party types inside the subsystem or adapter that owns
  them rather than exposing them through unrelated public APIs.

## Type And API Design

- Attach behavior to the type or subsystem that owns the relevant state. Prefer inherent methods
  or focused traits over unrelated free functions and scattered helpers.
- Prefer associated constructors and loaders for operations that create or recover a type, such
  as `ProjectInfo::from_manifest()` or `Layout::current()`.
- Design APIs around meaningful namespaces. Prefer `frames::allocate()` and `frames::release()`
  over flat names such as `allocate_frame()` and `release_frame()`.
- Model closed value sets with enums or bitflags instead of groups of integer constants. Keep
  invalid states unrepresentable when that does not make ordinary use cumbersome.
- Prefer strong types and domain-specific wrappers for addresses, identifiers, units, handles,
  states, and other values with distinct semantics or invariants. Accept raw integers or external
  representations only at the boundary that owns their validation, then convert immediately and
  keep internal APIs typed; do not introduce wrappers that add no distinction or validation.
- Use a newtype with inherent methods when a value has domain-specific invariants or behavior. Do
  not use a type alias if it would scatter validation and operations across helpers.
- Prefer a public field when callers may read or replace it directly without validation,
  normalization, side effects, or invariant maintenance. Do not write mechanical getters and
  setters solely to hide such a field; use methods when access must enforce behavior or preserve
  an invariant.

## Backend Selection

- When a subsystem has multiple compile-time-selectable backends, define one focused, preferably
  sealed backend trait and select the active implementation through adjacent `#[cfg]` type aliases.
  Keep selection in the owning adapter module; public wrappers and callers must depend on the
  trait contract rather than repeat conditional branches or expose concrete backend types:

  ```rust
  trait PageTableBackend { /* ... */ }

  #[cfg(target_arch = "x86_64")]
  type CurrentPageTableBackend = X86_64PageTableBackend;

  #[cfg(target_arch = "riscv64")]
  type CurrentPageTableBackend = Riscv64PageTableBackend;
  ```

## Ownership And Safety

- Express resource lifetimes through ownership, RAII, `Drop`, `Clone`, and lexical scopes. Avoid
  manual cleanup or reference bookkeeping when the type system can enforce the lifecycle.
- Name RAII guard types with a `Guard` suffix so their scoped lifetime and `Drop` behavior are
  visible at call sites.
- Prefer a smaller lexical scope over an explicit `drop()`. Use explicit `drop()` only when the
  smaller scope would be artificial or less readable.
- Keep unsafe code local. Document every unsafe block or implementation with an adjacent `SAFETY`
  explanation of its invariants and caller obligations.

## Expression Style

- Use the newest appropriate Rust and standard-library features supported by the pinned nightly
  toolchain. Do not retain compatibility patterns for older compilers.
- Prefer chained transformations, iterators, and combinators when ownership, failure behavior, and
  control flow remain obvious. Break chains at semantic boundaries or where error context becomes
  unclear.
- Use `tap` to configure a value that is immediately returned or passed onward. Do not introduce a
  `let mut value; ...; value` pattern solely for configuration.
- Avoid turbofish syntax when a local type annotation communicates the same information more
  clearly. Use turbofish when inference requires it or it materially improves readability.
- At call sites, prefer `.into()` over `Target::from(value)`. Implement `From`, not `Into`, for
  repository-owned conversions. Add a type annotation or turbofish if `.into()` hides the target.
- Prefer expressive code over explanatory comments. Comments should capture rationale,
  invariants, non-obvious constraints, or safety obligations rather than restating the code.
- Comments may refer directly to function parameters or local variables by name. For example,
  `take_waiter_thread(&mut self, process_id: ProcessID)` may say: "Returns the thread ID of the
  process waiting for `process_id` to exit."
- When a module, type, ownership boundary, lifecycle, registration path, or control/data-flow
  structure is hard to understand from the code alone, add a focused nearby comment explaining
  how the pieces fit together. Assume readers have not read `DESIGN.md`; source code plus its
  comments must be sufficient to understand the local structure.

## Blank Lines

- Use blank lines to divide semantic steps such as validation, registration, construction, unsafe
  side effects, and finalization. Keep statements belonging to one continuous operation together.
- Separate a multi-line block-bodied statement from surrounding independent statements with one
  blank line before and after it. This applies to `if`, `match`, `for`, `while`, `loop`, `unsafe`,
  and standalone blocks. Do not require a blank line at the beginning or end of a function body,
  before `else`, between `match` arms, or when the following syntax continues the same expression.
- Separate the final independent statement from a function's return expression with one blank
  line. For example, write `foo();` followed by a blank line before `Ok(())`.
- Group consecutive operations of the same kind, such as `let` bindings, function calls, or
  assignments, together. Surround each group with blank lines when it is separated from another
  kind of operation or semantic step.
- Surround a multi-line expression or method chain with blank lines when it is an independent
  operation. For example, separate `addrspace.read_bytes(...).map_err(...)?;` from the statements
  before and after it, even when rustfmt places the chain across several source lines.

## Imports And Paths

- Import an item before use instead of repeating its full path. Prefer `use foo::bar::baz;` followed
  by `baz()` over `foo::bar::baz()`.
- Preserve qualification when it is part of the API vocabulary or prevents ambiguity. For example,
  shorten `memory::frames::allocate()` to `frames::allocate()`, not to `allocate()`.
- Do not use wildcard imports outside a deliberately designed prelude.
- Group imports according to rustfmt output.

## Dependencies And Abstraction

- Before implementing functionality, check whether an existing crate already provides it. Prefer
  a maintained, compatible crate over a repository-local implementation.
- Evaluate crates for `no_std` and target support, licensing, maintenance, soundness, and API fit.
- Declare every dependency in the root `[workspace.dependencies]` table, including repository-owned
  `roxy-*` crates. Member crates must inherit them with `dependency.workspace = true` rather than
  repeating versions, paths, features, or other dependency configuration locally.
- Use broad compatible version requirements for workspace dependencies. Let `Cargo.lock` pin exact
  resolved versions.
- In each dependency table, keep third-party dependencies in one alphabetized group and `roxy-*`
  dependencies in a separate alphabetized group. Put the third-party group first and separate the
  groups with one blank line.
- Use macros when they remove genuine repetition or enforce an invariant. Prefer functions, traits,
  and ordinary types when they offer equivalent clarity and diagnostics.
- Keep macros small, hygienic, and narrowly scoped. They must not conceal important control flow,
  unsafe operations, allocation, locking, or I/O.

## TOML Style

- Prefer dotted keys over inline tables for a single nested value. Write `foo.bar = "baz"` instead
  of `foo = { bar = "baz" }`.
- Keep an inline table when several fields form one compact value and dotted keys would make the
  relationship harder to scan.

## Refactoring And Incomplete Work

- Complete repository-wide refactors in one change: migrate every caller and remove obsolete
  paths. Keep old and new implementations in parallel only under an explicit migration plan.
- Mark intentionally incomplete behavior with a specific `TODO` or `FIXME` describing what is
  missing and, when useful, the condition for removing it.
- Never hide missing behavior behind a silent fallback or a misleading successful result.
