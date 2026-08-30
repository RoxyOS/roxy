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

## Unsupported Requests

The repository-wide rule (see `agent-instructions/GENERAL.md`) is that **no** userspace request may
be silently degraded or silently ignored because kernel functionality is missing or incomplete.
Every such path must emit the centralized `UNSUPPORTED` diagnostic before returning, via the
`crate::unsupported::unsupported_argument` helper (through the per-syscall `unsupported()` shim).

This applies equally to:

- Whole unsupported syscalls and partially implemented commands.
- Unsupported **flag bits or option values** on a supported syscall, even when POSIX/Linux
  semantics would permit silently ignoring them. "Linux ignores it" is **not** a justification to
  skip the diagnostic here.

Concretely:

- Do **not** mask out unknown or unimplemented bits with `from_bits_retain` + a supported-bit
  mask and drop them silently. If a flag can reach the handler but is not implemented, it must be
  routed to `unsupported()`.
- When a command legitimately accepts a bit-mask of mixed supported/unsupported flags (e.g.
  `fcntl(F_SETFL)`), still report every unsupported bit through `unsupported()` — report once per
  unsupported bit — while continuing to apply the supported bits and returning success. Do not
  drop unknown bits silently.
- Keep `unsupported()` reporting inside the `SyscallArg::parse` implementation for flag-word
  arguments (matching the `OpenFlags` pattern), and in the handler for command values.

Use the `unsupported()` shim's `ENOTSUP` errno by default; only use a different errno (such as
`EINVAL`) when the ABI contract for that argument demands it (e.g. callers retry on `EINVAL`).

## Argument Parsers

- Put a `SyscallArg` implementation for any argument type that can be shared by multiple syscalls
  in `src/args/`. Syscall-local argument parsers are reserved for layouts or semantics unique to
  that syscall.
- Syscall handlers receive parsed domain values only. Do not use `raw_*` parameters or decode raw
  ABI words in handler bodies.
- Represent every flags / bit-mask argument with a `bitflags!` type that implements `SyscallArg`,
  following the `OpenFlags` pattern in `syscalls/open.rs`. Do not decode flag words with hand-written
  integer constants and `&`/`!` arithmetic in handler bodies; keep unknown-bit rejection inside the
  `SyscallArg::parse` implementation.
