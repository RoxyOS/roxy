# `fbdev` Design

## Purpose and scope

`roxy-fbdev` is the framebuffer character device driver. It exposes the boot framebuffer that
`roxy-fbterm` validated as `/dev/framebuffer`: it answers one query for the layout and pixel
format, it describes the framebuffer's physical memory so userspace can `mmap` it, and it hands the
visible frame to one client process at a time. It does not own framebuffer validation, text
rendering, mode setting, or the physical mapping itself: `roxy-fbterm` owns the layout and the
console's pixels, and `roxy-vm` installs user mappings.

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
statically and has no mutable state of its own.

Control of the visible frame is process-wide state in the `claim` module rather than a field of the
device object, because there is exactly one boot framebuffer and one device node for it, and
because the release path must be reachable from a function-pointer notification. `register` also
installs `claim::release_exited` as the process-exit notification, so ownership cannot outlive the
process that took it.

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
- `TAKE_CONTROL` (request `1`) and `RELEASE_CONTROL` (request `2`) move ownership of the visible
  frame. Taking it suspends framebuffer terminal drawing, so the kernel console stops painting over
  a client's pixels, and releasing it resumes drawing on a cleared screen. The request argument is
  ignored.
- `mmap` accepts only `offset == 0` with `size` up to `memory_length` rounded up to a whole page
  (userspace mmaps at page granularity), and only when the framebuffer address is page-aligned. It
  returns the physical range without copying or retaining any reference, because the mapping lives
  for the kernel lifetime. `mmap` is the only path by which pixels reach userspace.
- Unknown typed requests return `IoctlError::Unsupported`; the syscall layer reports them through
  the centralized unsupported-operation diagnostic.

The wire record and request numbers are defined by `kernel/syscall`, and are mirrored by
`sysdeps/roxy/include/roxy/framebuffer-dev.h` in the Roxy mlibc fork. The device deliberately has
no mode-setting request: the boot loader owns the mode, so a client that wants to know whether a
mode is usable compares it against what `GET_INFO` reported, and a client that wants to keep using
the current mode simply does not ask for anything.

Control follows the process, not the descriptor. Any thread of the holder may release the frame, a
repeated take from the holder succeeds so that a client can assert ownership, and a take by another
process fails. Mapping is not gated on control: an unowned frame is already readable and writable
through `mmap`, so control expresses "the console must not draw", not "only the holder may touch
the pixels". Error selection follows the DRM master ioctls (`drivers/gpu/drm/drm_auth.c`): taking a
frame another process holds fails with `EBUSY`, and releasing one the caller does not hold fails
with `EINVAL`.

Only `roxy-fbdev` knows about userspace requests, so it is also the only place that couples a
request to terminal behavior: `roxy-fbterm` exposes `suspend_drawing` and `resume_drawing` and knows
nothing about who calls them.

## Limits

Blanking, panning, palette, cursor, double-buffer control, and mode changes are absent rather than
rejected with a special errno: a client that sends their numbers receives the same `ENOTTY` as any
other unknown request.

The client owns the pixels it draws while it holds the frame; the kernel does not validate,
restore, or preserve them. Releasing the frame clears the screen, because the console keeps no text
model to repaint from, so content written to the console while the frame was held is lost from the
display. A client that exits without releasing the frame is still released, by the process-exit
notification, so a crashed client cannot leave the console suspended permanently.
