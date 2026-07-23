# PS/2 Keyboard Design

## Purpose and scope

`roxy-ps2` owns the x86_64 i8042 first port and ISA IRQ1. It converts Scan Code Set 1 keyboard
traffic from a US 104-key keyboard into a bounded `roxy-input::InputDevice` stream of character and
special-key events. It does not implement a terminal line discipline, echo, terminal attributes,
control signals, layout selection, mouse input, or the i8042 second port.

The driver uses `pc-keyboard` 0.9 with `PS2Keyboard<Us104Key, ScancodeSet1>` and
`HandleControl::Ignore`. That crate is `no_std`, is licensed under MIT or Apache-2.0, maintains the
stateful Set 1 and modifier mapping required here, and keeps third-party decoder types inside this
subsystem. Character key presses become Unicode events; releases for character keys and unsupported
raw keys are discarded. Navigation and function keys retain pressed/released state as repository-owned
key codes. Enter, backspace, tab, space, escape, letters, digits, and basic punctuation remain
character events; Ctrl does not synthesize control characters.

## Initialization and hardware ownership

Core initializes the driver after the interrupt controller and scheduler consumers are ready but
before the periodic timer and global interrupts start. Initialization disables the first i8042
port, drains pending output, enables IRQ1 and Set 1 translation in the controller configuration,
reenables the port, resets the keyboard, and enables scanning. Only after both keyboard handshakes
succeed does the driver register its IRQ1 handler and unmask the route.

The supported platform is required to provide this controller and route. Every controller wait is
bounded; a command timeout, unexpected reset response, or failed scanning acknowledgement is a
boot-fatal initialization error. The driver does not probe for an alternate input device or
silently fall back to an output-only framebuffer terminal.

## Buffering

Decoded input enters a fixed-capacity `heapless::Deque<InputEvent, 256>`. IRQ handling never allocates or
blocks. When the queue is full, the arriving byte is discarded without additional accounting;
queued events retain their order. `InputDevice::read_event` removes one oldest event or returns
`None` without waiting. It does not encode bytes, interpret a caller buffer, wait for interrupts, or
apply terminal semantics; the TTY FD adapter owns those policies.

## Interrupt and locking contract

The IRQ handler reads port `0x60` before taking driver state, then decodes and enqueues under the
driver lock. It runs with interrupts disabled and must not allocate, block, switch threads, or
retain terminal, process, descriptor, or scheduler locks across device I/O. The interrupt
subsystem owns EOI delivery; the handler has the common `roxy_interrupt::Handler = fn()` signature
and returns no disposition.

The current implementation assumes the repository's single-BSP execution model. PS/2 mouse IRQ12,
second-port discovery, non-US layouts, complete extended-key behavior, LED synchronization,
hot-plugging, canonical mode, echo, termios, and signals remain outside this subsystem.
