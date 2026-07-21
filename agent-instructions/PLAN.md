# Plan Mode Instructions

- Plans must be implementation-ready and sufficiently detailed that another engineer can execute
  them without rediscovering the intended design.
- Begin by stating the objective, current behavior, desired behavior, scope, explicit non-goals,
  constraints, assumptions, and measurable completion criteria.
- Before proposing implementation steps, inspect the relevant code paths, applicable `AGENTS.md`,
  `agent-instructions/STYLE.md`, subsystem `DESIGN.md` files, tests, manifests, and related history
  when necessary.
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
