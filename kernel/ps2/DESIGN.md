# PS/2 Input Design

## Purpose and scope

`roxy-ps2` owns the x86_64 i8042 controller: both the first port (keyboard, ISA IRQ1) and the
second port (mouse, ISA IRQ12). It does not buffer events, register listeners, implement a
terminal line discipline, echo, terminal attributes, control signals, or layout selection.

### Keyboard (first port)

The keyboard path converts Scan Code Set 1 traffic from a US 104-key keyboard into raw `KeyEvent`
records and publishes them to the process-wide keyboard manager (`roxy-keyboard-input`).  The
driver uses `pc-keyboard` 0.9 for scan-code parsing only: `ScancodeSet1` turns bytes into
`KeyEvent` records.  The stateful layout decoder (`EventDecoder`, layouts, modifier state) is
deliberately **not** used here; character and layout mapping belongs to consumers such as
`roxy-tty`.  Every physical key press and release — including modifiers and character-key
releases — becomes a `KeyCode` event.  No control characters are synthesised and no Unicode
characters are produced.

### Mouse (second port)

The mouse path decodes raw PS/2 data bytes into semantic `MouseEvent` batches and publishes them
to the process-wide mouse manager (`roxy-mouse-input`).  The driver also preserves the legacy
`/dev/psaux` path, which queues raw bytes for consumers that parse the PS/2 protocol in
userspace.

## Initialization and hardware ownership

### Keyboard

Core initializes the driver after the interrupt controller and scheduler consumers are ready but
before the periodic timer and global interrupts start.  Initialization disables the first i8042
port, drains pending output, enables IRQ1 and Set 1 translation in the controller configuration,
re-enables the port, resets the keyboard, and enables scanning.  Only after both keyboard
handshakes succeed does the driver register its IRQ1 handler and unmask the route.

The supported platform is required to provide this controller and route.  Every controller wait
is bounded; a command timeout, unexpected reset response, or failed scanning acknowledgement is
a boot-fatal initialization error.  The driver does not probe for an alternate input device or
silently fall back to an output-only framebuffer terminal.

### Mouse

Initialization of the second port happens after the keyboard is ready (but still before global
interrupts are enabled).  The sequence is:

1. Enable the second PS/2 port, set IRQ2 in the controller configuration.
2. Reset the mouse (`0xFF`), verify the self-test (`0xAA`) and the initial device ID (standard
   mice report `0x00`).
3. Probe for the IntelliMouse Z-axis extension by quickly setting the sample rate to 200, 100,
   80, then reading the device ID (`0xF2`).  If the ID is `0x03`, the mouse is an IntelliMouse
   and uses 4-byte packets (with a Z-axis wheel byte); otherwise it falls back to standard
   3-byte packets.
4. Enable data reporting (`0xF4`).
5. Register the IRQ12 handler and unmask the route.

A missing or failed mouse is tolerated: the controller may simply have no second port (common
on real hardware without an auxiliary device), so failure is reported to the caller rather than
panicking.  The `/dev/psaux` node is always registered regardless of attachment.

## Event publishing

### Keyboard

The IRQ1 handler reads port `0x60` before taking the parser lock, then parses the scancode byte
into a `KeyEvent`.  If the parse succeeds, the handler calls `roxy_keyboard_input::publish(key)`,
which broadcasts the event to every registered input listener (TTY, evdev, …).  The handler does
not queue, buffer, or filter events; it produces at most one `KeyEvent` per IRQ.

### Mouse

The IRQ12 handler reads port `0x60` and sends the byte to **two** consumers in sequence:

1. The legacy `psaux` byte queue (`/dev/psaux`), which preserves the raw byte stream for
   userspace PS/2 drivers.
2. The `MousePacketParser` state machine, which accumulates bytes into 3- or 4-byte packets
   (depending on the negotiated mode) and, on each complete packet, produces a `Vec<MouseEvent>`
   batch.  Non-empty batches are published via `roxy_mouse_input::publish(events)`.

Both handlers run with interrupts disabled and must not allocate, block, switch threads, or
retain terminal, process, descriptor, or scheduler locks across device I/O.  The interrupt
subsystem owns EOI delivery; the handler has the common `roxy_interrupt::Handler = fn()`
signature and returns no disposition.

### Packet decoding

The mouse packet decoder (`packet.rs`) implements the standard PS/2 mouse protocol and the
IntelliMouse Z-axis extension.  Decoding behaviour follows Linux
`drivers/input/mouse/psmouse-base.c` as a cross-check:

- X and Y are 9-bit signed deltas: `x = byte1 - ((byte0 << 4) & 0x100)`,
  `y = byte2 - ((byte0 << 3) & 0x100)`; a zero delta byte yields zero motion.
- The Y delta is negated (`down = -y`) so that screen-down is positive, matching evdev's
  `REL_Y` convention.
- The IntelliMouse Z-axis byte is sign-extended and negated
  (`up = -(s8)byte3`), so that a positive `Scroll { up }` means upward scrolling.
- Buttons are the low three bits of byte0 (bit0 left, bit1 right, bit2 middle).  A `MouseEvent`
  is emitted only when a button's state changes between packets.
- Overflow bits (`yo`/`xo`) are ignored, matching Linux's standard motion report.

## Consumer contract

### Keyboard

Each consumer (TTY, evdev-keyboard) registers with the keyboard manager via
`roxy_keyboard_input::register_listener(listener)`.  Registration happens at boot, before
interrupts are enabled.  The consumer owns its own bounded queue and decides whether to process
events immediately (IRQ-path for signal responsiveness) or defer to a read/wait path.

### Mouse

Each consumer (evdev-mouse, future TTY mouse support) registers with the mouse manager via
`roxy_mouse_input::register_listener(listener)`.  Registration happens at boot, before
interrupts are enabled.  The listener receives a batch of `MouseEvent` records per hardware
sample, and the consumer decodes them into its own internal representation or pushes them into
a bounded queue.

## Limits

The controller and its IRQ1/IRQ12 handlers run on the single CPU that the IOAPIC routes these
legacy lines to (the BSP), so the driver keeps one owner and no per-CPU or lock-free controller
state. Non-US keyboard layouts, complete extended-key behaviour, LED synchronization,
hot-plugging, canonical mode, echo, termios, and signals remain outside this subsystem. PS/2
mouse support targets standard 3-button mice and IntelliMouse (wheel) mice; IntelliMouse Explorer
(5-button, ID 4) and other vendor extensions are not yet implemented.