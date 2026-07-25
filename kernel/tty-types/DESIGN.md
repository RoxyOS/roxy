# TTY Types Design

## Purpose and scope

`roxy-tty-types` owns layout-neutral TTY domain values shared by terminal endpoints, the
descriptor layer, TTY implementation, and syscall ABI translation. It exists so `roxy-fd` can
expose typed ioctl requests without depending on a concrete TTY implementation, and so
`roxy-tty` can implement those requests without importing terminal state types from `roxy-fd`.

The crate does not own userspace ABI layouts, ioctl request numbers, raw userspace pointers, file
descriptor state, input devices, output rendering, line discipline behavior, or terminal
initialization.

## Dependency boundary

TTY ioctl domain types flow across three layers:

- `roxy-syscall` decodes personality-specific ioctl numbers and `#[repr(C)]` ABI records into
  these values.
- `roxy-fd` embeds these values in `IoctlRequest` so every file object receives one typed dispatch
  surface.
- `roxy-tty` applies the values to its line discipline and shared window-size state.

The types in this crate are kernel-domain values rather than ABI records. They must remain
independent of userspace layout: no `#[repr(C)]`, explicit padding, request numbers, errno policy,
or raw userspace pointers belong here. Adding another syscall personality should add or adjust ABI
translation under `roxy-syscall`, not change the representation in this crate unless the kernel
domain itself changes.

`LocalFlags` models the known terminal local-mode bits while retaining unknown bits from an ABI
decoder. The syscall layer must use `from_bits_retain` and `bits` for conversion; the concrete
TTY decides which bits it supports and reports unsupported values through its ioctl error path.

`Termios` is the complete TTY ioctl value. It is distinct from
`roxy-line-discipline::LineDisciplineSettings`, which contains only the input-policy settings the
line discipline can execute.
