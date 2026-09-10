# Mouse Device Design

## Purpose and scope

`roxy-mouse-dev` owns `/dev/mouse`: the character device that carries pointer samples from the
PS/2 mouse driver to userspace. It defines the wire record, the bounded queue that decouples the
IRQ producer from readers, and the poll readiness that goes with it.

It does not own hardware access or PS/2 protocol decoding — those belong to `roxy-ps2`. It does
not own pointer acceleration, cursor state, or button remapping; those belong to consumers (the X
mouse driver in userspace).

Explicit non-goals: absolute axes (touchpads, tablets), multiple pointing devices, button
remapping, and any ioctl interface. The device serves data through `read` only.

## The wire contract

`RoxyMouseEvent` is a 24-byte `repr(C)` record: a `CLOCK_REALTIME` timestamp in nanoseconds,
relative motion, the wheel delta of the sample, and the button state after the sample.

Two decisions shape consumer code:

- **One hardware sample becomes exactly one record.** Motion, wheel and button changes that arrive
  in the same PS/2 packet are folded together, so a reader never has to reassemble a batch or look
  for a batch terminator.
- **`buttons` is state, not a delta.** Consumers diff successive records to find transitions. A
  reader that starts mid-stream therefore needs no handshake: the first record carries the whole
  button state.

This is a **device-serialised protocol record**, not a direct syscall ABI argument, so it lives
here rather than in `roxy-syscall` (AGENTS.md "Design and Safety"). The layout and the
`ROXY_MOUSE_BTN_*` bits are a hand-maintained contract with
`sysdeps/roxy/include/roxy/mouse-dev.h` in the Roxy mlibc fork; both sides change together.
`button_bit` is an exhaustive match over `MouseButton`, so a new button cannot silently miss its
ABI bit.

## Data flow

```
IRQ12 → roxy-ps2 packet parser → roxy-mouse-input::publish(&[MouseEvent])
      → MouseDevice::on_receive_input → fold sample → queue → read / poll
```

Registration happens at boot: `kernel-main` creates the device with the wheel capability reported
by `roxy-ps2::mouse_has_wheel()`, registers it with devfs under the name `mouse`, and registers
the returned listener with `roxy-mouse-input`, keeping the `Arc` alive for the kernel lifetime.

## Concurrency

`on_receive_input` runs in IRQ context. It takes the button-state lock for the duration of one
sample and the queue lock while enqueuing, never allocates, and never sleeps. The button state is
carried across samples so each record can report the whole state; the two locks are never taken in
the opposite order anywhere, so no inversion is possible. A full queue drops its oldest record so
the producer cannot block.

## Failure behaviour

`read` returns `EINVAL` (`FileError::BadOperation`) when the buffer cannot hold one whole record.
It drains as many whole records as fit and returns `EAGAIN` (`WouldBlock`) when nothing was queued.
`poll` reports `readable` exactly when the queue is non-empty. On a mouse without a wheel, scroll
input is dropped instead of reported, so `wheel` is always zero.

## Limits

One pointing device, relative motion only. Absolute-axis devices would need their own record kind
and a capability signal to userspace, neither of which exists yet.
