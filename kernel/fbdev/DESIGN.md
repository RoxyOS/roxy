# `fbdev` Design

## Purpose and scope

`roxy-fbdev` is the framebuffer character device driver. It exposes the boot framebuffer that
`roxy-fbterm` validated as `/dev/fb0`: it answers `FBIOGET_VSCREENINFO`/`FBIOGET_FSCREENINFO`
queries and describes the framebuffer's physical memory for userspace `mmap`. It does not own
framebuffer validation, text rendering, mode setting, or the physical mapping itself; `roxy-fbterm`
owns the layout and `roxy-vm` installs user mappings.

## Ownership and registration

`roxy-fbterm` publishes a `FramebufferLayout` (physical address, dimensions, pitch, bits per
pixel, and RGB channel bit placement) exactly once after its mode validation succeeds. The
composition root calls `roxy_fbdev::register` with the shared `DeviceRegistry`; the function
registers `FramebufferDevice` under `fb0` only when a layout exists, so serial-only or
unsupported-mode boots expose no device. `FramebufferDevice` borrows the published layout
statically and is therefore stateless: it cannot observe or mutate the terminal renderer.

## Contract

Layout-to-screen-info conversion lives in a dedicated `convert` module: `var_info` and
`fixed_info` turn a `FramebufferLayout` into the neutral `FbVarInfo`/`FbFixedInfo` values, and
`memory_length` derives the one-frame byte length. The device implementation delegates to these
functions, so the conversion is testable without the ioctl dispatch path.

- Metadata reports a character device with mode `0600` and a stable file ID.
- `FBIOGET_VSCREENINFO` reports the visible and virtual resolution, bits per pixel, and the
  RGB channel offsets/lengths from the validated layout; all timing, margin, activation, and
  reserved fields report zero.
- `FBIOGET_FSCREENINFO` reports the physical framebuffer address, the memory length
  (`pitch × height`), packed-pixels type, truecolor visual, and the pitch as `line_length`.
- `FBIOPUT_VSCREENINFO` mirrors fixed-mode Linux drivers: the boot loader owns the mode, so a
  request describing the current mode (including `FB_ACTIVATE_TEST` probes) succeeds as a
  no-op, and any actual mode change is rejected with `EINVAL`. The user buffer is never
  written back, so callers observe the mode exactly as requested.
- `mmap` accepts only `offset == 0` with `size` up to `smem_len` rounded up to a whole page
  (userspace mmaps at page granularity), and only when the framebuffer address
  is page-aligned; it returns the physical range without copying or retaining any reference,
  because the mapping lives for the kernel lifetime.
- Unknown typed requests return `IoctlError::Unsupported`; the syscall layer reports them through
  the centralized unsupported-operation diagnostic.

## Limits

Mode changes (`FBIOPUT_VSCREENINFO` with values other than the current mode), panning,
palette, and double-buffer control are unsupported and rejected. The device does not coordinate with the framebuffer
terminal: userspace drawing and terminal rendering write the same memory, so taking the device
over for graphics requires a future exclusive-mode switch outside this crate's scope.
