# Syscall Subsystem Style

This guide supplements the repository-wide `agent-instructions/STYLE.md` for syscall handlers.

## Handler Flow

Keep every syscall handler in three visibly separated stages, with one blank line between stages:

1. Parse raw arguments into typed local values.

   ```rust
   let fd = ...;
   let count = ...;
   ```

2. Check bounds, user addresses, permissions, descriptor state, and other preconditions.

3. Run the actual syscall implementation after validation succeeds.

Do not mix argument conversion, validation side effects, and the operation itself in one tightly
packed block. Small early returns for invalid arguments belong to the parsing or checking stage;
the actual implementation starts only after those checks have completed.

Keep argument parsing and checking in the syscall handler, but keep the actual implementation to
10 lines or fewer. If the implementation exceeds 10 lines, move it to the owning subsystem and
call it from the handler after validation succeeds.

## Argument Parsers

- Put a `SyscallArg` implementation for any argument type that can be shared by multiple syscalls
  in `src/args/`. Syscall-local argument parsers are reserved for layouts or semantics unique to
  that syscall.
- Syscall handlers receive parsed domain values only. Do not use `raw_*` parameters or decode raw
  ABI words in handler bodies.
