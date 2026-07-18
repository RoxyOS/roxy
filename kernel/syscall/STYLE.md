# Syscall Subsystem Style

This guide supplements the repository-wide `STYLE.md` for syscall handlers.

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
