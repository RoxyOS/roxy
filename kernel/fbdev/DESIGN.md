# `fbdev` Design

## Purpose and scope

`roxy-fbdev` is the framebuffer character device driver. It exposes the boot framebuffer that
`roxy-fbterm` validated as `/dev/framebuffer`: it answers one query for the layout and pixel
format, and it describes the framebuffer's physical memory so userspace can `mmap` it. It does not
own framebuffer validation, text rendering, mode setting, or the physical mapping itself;
`roxy-fbterm` owns the layout and `roxy-vm` installs user mappings.

The device is deliberately not the Linux fbdev interface. Roxy defines its own single-record
protocol so that no physical address and no timing state crosses the boundary, and so that the
userspace ABI is documented in one place (`roxy/framebuffer-dev.h`) instead of in the Linux
header's wide, mostly-unused record pair.

## Ownership and registration

`roxy-fbterm` publishes a `FramebufferLayout` (physical address, dimensions, pitch, bits per
pixel, and RGB channel bit placement) exactly once after its mode validation succeeds. The
composition root calls `roxy_fbdev::register` with the shared `DeviceRegistry`; the function
registers `FramebufferDevice` under `framebuffer` only when a layout exists, so serial-only or
unsupported-mode boots expose no device. `FramebufferDevice` borrows the published layout
statically and is therefore stateless: it cannot observe or mutate the terminal renderer.

## Contract

Layout-to-description conversion lives in a dedicated `convert` module: `info` turns a
`FramebufferLayout` into the neutral `FbInfo`, and `memory_length` derives the one-frame byte
length. The device implementation delegates to these functions, so the conversion is testable
without the ioctl dispatch path.

- Metadata reports a character device with mode `0600` and a stable file ID.
- `GET_INFO` (request `0`) reports width, height, stride, the one-frame byte length, and the three
  RGB channel `size`/`shift` pairs. Pixels are always 32 bits wide and always in the RGB memory
  model, because `roxy-fbterm` publishes no device for any other layout, so the record carries no
  bit-depth, visual, timing, margin, or physical-address field.
- `mmap` accepts only `offset == 0` with `size` up to `memory_length` rounded up to a whole page
  (userspace mmaps at page granularity), and only when the framebuffer address is page-aligned. It
  returns the physical range without copying or retaining any reference, because the mapping lives
  for the kernel lifetime. `mmap` is the only path by which pixels reach userspace.
- Unknown typed requests return `IoctlError::Unsupported`; the syscall layer reports them through
  the centralized unsupported-operation diagnostic.

The wire record and request number are defined by `kernel/syscall`, and are mirrored by
`sysdeps/roxy/include/roxy/framebuffer-dev.h` in the Roxy mlibc fork. The device deliberately has
no mode-setting request: the boot loader owns the mode, so a client that wants to know whether a
mode is usable compares it against what `GET_INFO` reported, and a client that wants to keep using
the current mode simply does not ask for anything.

## Limits

The device has exactly one request. Panning, palette, blanking, cursor, double-buffer control, and
mode changes are absent rather than rejected with a special errno: a client that sends their
numbers receives the same `ENOTTY` as any other unknown request, which is what `xorg-server`'s
fbdev paths and any other client already treat as "feature unavailable".

The device does not coordinate with the framebuffer terminal: userspace drawing and terminal
rendering write the same memory, so taking the device over for graphics requires a future
exclusive-mode switch outside this crate's scope.
