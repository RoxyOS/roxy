# PS/2 Keyboard Design

## Purpose and scope

`roxy-ps2` owns the x86_64 i8042 first port and ISA IRQ1. It converts Scan Code Set 1 keyboard
traffic from a US 104-key keyboard into raw `KeyEvent` records and publishes them to the
process-wide input manager. It does not buffer events, register listeners, implement a terminal
line discipline, echo, terminal attributes, control signals, layout selection, mouse input, or
the i8042 second port.

The driver uses `pc-keyboard` 0.9 for scan-code parsing only: `ScancodeSet1` turns bytes into
`KeyEvent` records. The stateful layout decoder (`EventDecoder`, layouts, modifier state) is
deliberately **not** used here; character and layout mapping belongs to consumers such as
`roxy-tty`. Every physical key press and release — including modifiers and character-key
releases — becomes a repository-owned `KeyCode` event. No control characters are synthesized
and no Unicode characters are produced.

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

## Event publishing

The IRQ handler reads port `0x60` before taking the parser lock, then parses the scancode byte
into a `KeyEvent`. If the parse succeeds, the handler calls `roxy_input::publish(key)`, which
broadcasts the event to every registered input listener (TTY, evdev, …). The handler does not
queue, buffer, or filter events; it produces at most one `KeyEvent` per IRQ.

The handler runs with interrupts disabled and must not allocate, block, switch threads, or retain
terminal, process, descriptor, or scheduler locks across device I/O. The interrupt subsystem owns
EOI delivery; the handler has the common `roxy_interrupt::Handler = fn()` signature and returns
no disposition.

## Consumer contract

Each consumer (TTY, future evdev) registers with the input manager via
`roxy_input::register_listener(listener)`. Registration happens at boot, before interrupts are
enabled. The consumer owns its own bounded queue and decides whether to process events immediately
(IRQ-path for signal responsiveness) or defer to a read/wait path.

## Limits

The current implementation assumes the repository's single-BSP execution model. PS/2 mouse IRQ12,
second-port discovery, non-US layouts, complete extended-key behavior, LED synchronization,
hot-plugging, canonical mode, echo, termios, and signals remain outside this subsystem.