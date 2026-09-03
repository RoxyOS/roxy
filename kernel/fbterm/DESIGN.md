# `fbterm` Design

## Purpose and scope

`roxy-fbterm` provides the framebuffer-backed implementation of the shared terminal output
endpoint. It owns text rendering state and framebuffer writes; it does not own boot protocol
parsing, process descriptors, keyboard hardware, input waiting, terminal line discipline, or kernel
diagnostics.

## Ownership and initialization

In normal builds, core initializes `fbterm` after boot metadata and kernel memory are ready. Kernel
test builds select serial directly and do not require framebuffer initialization. The output uses
the first Limine framebuffer and its HHDM virtual address for the kernel lifetime. Unsupported modes
are reported to core, which selects the serial terminal instead. A successful initialization
publishes the output `Arc` exactly once; a
mode-validation failure leaves the global endpoint uninitialized, while a second successful
initialization attempt violates the core startup contract and panics.

Initialization also publishes a `FramebufferLayout` once: the physical address, dimensions, pitch,
bits per pixel, and RGB channel placement of the validated framebuffer. This neutral description is
the contract between `fbterm` (the layout owner) and device drivers such as `roxy-fbdev` that expose
the framebuffer to userspace. The layout is captured before the `Framebuffer` moves into the
renderer and lives for the kernel lifetime; terminal-only builds never publish it.

## Terminal behavior

The endpoint renders printable ASCII with the regular Terminus 8x16 bitmap font from
`embedded-bitmap-fonts`. Only that crate's `terminus` feature is enabled. It is `no_std`, uses the
Apache-2.0 license, and provides a fixed bitmap whose dimensions match the cell renderer without
font parsing, allocation, or scaling. LF advances a row, CR returns to column zero, backspace
clears the preceding cell, and tab advances to the next eight-column stop. Reaching the bottom
scrolls the framebuffer by one glyph row. Input is supplied by `roxy-keyboard-input` implementations through
`roxy-tty`; the framebuffer console itself only owns output rendering.

Only Limine RGB 32-bit modes are accepted. Color masks determine packed foreground/background
pixels. The console uses `vte` without its standard-library or ANSI semantic features to parse the
output stream. Parser state persists across writes, while `fbterm` owns the meaning of supported
events. Printable ASCII and C0 LF, CR, backspace, and tab retain their direct terminal behavior.
CSI supports relative and absolute cursor movement, display and line erasure, cursor save and
restore, cursor visibility, and SGR default plus standard and bright 16-color foreground and
background selection. ESC `7` and `8` also save and restore the cursor. Coordinates are clamped to
the text grid, and malformed, unknown, OSC, and DCS sequences are ignored without rendering their
control bytes.

Unicode glyphs, text attributes, indexed and true color, insertion and deletion, scroll regions,
alternate screens, terminal replies, input devices, PTYs, and diagnostic mirroring are outside the
current contract. Consequently this subset targets ordinary shell output rather than full-screen
ncurses application compatibility.

## Rendering model

Rendering is split into three ownership layers:

```text
vte Parser → Screen → TextRenderer → Framebuffer → framebuffer mapping
```

`Framebuffer` is the pixel-addressed hardware boundary. It validates and owns the boot-provided
mapping, physical pixel dimensions, pitch, and RGB channel layout. It converts RGB components into
native pixels and provides bounded pixel, rectangle, and pixel-row operations. It has no concept of
characters, cells, cursors, or terminal control bytes. Every pixel access is a volatile 32-bit
operation so scrolling does not turn the device mapping into an ordinary-memory copy.

`TextRenderer` interprets the framebuffer as a grid of fixed 8x16 cells. It derives and owns the
grid's total `columns` and `rows` from the framebuffer dimensions; a partial cell at the right or
bottom edge is outside the text grid. The renderer owns the foreground and background colors and
adapts Terminus's binary glyph output to pixels inside one selected cell. The adapter remains
private to the rendering layer, so neither the framebuffer boundary nor the terminal state machine
depends on third-party graphics types. The renderer can draw or clear a cell, reversibly invert a
selected cell for cursor display, and scroll the complete text region upward by one cell row. It
does not own a current cell, advance a cursor, or interpret control bytes.

`Console` owns the persistent `vte` parser and one `Screen`. `Screen` implements `vte::Perform`,
owns current and saved cursor positions plus cursor visibility, and translates supported parser
events into renderer operations. Printable ASCII draws into the current cell and advances the
cursor. Reaching the final column wraps to the next row. LF selects column zero of the next row, CR
selects column zero without changing rows, backspace moves left and clears that cell when possible,
and tab emits spaces until the next eight-column stop. Moving beyond the last row asks
`TextRenderer` to scroll one cell row and leaves the cursor on the new final row.

The current cell is shown with a reversible full-cell XOR cursor while the console is idle;
batched writes hide it before parsing and restore it at the final position unless ANSI mode state
hides it. The XOR mask is the framebuffer-native packed white value rather than a function of the
current SGR colors. This makes cursor removal restore any previously rendered cell exactly even
after the active foreground or background changes.

The output path is therefore:

```text
input bytes
  → vte preserves parsing state and emits terminal events
  → Screen applies the supported semantics and selects cells or regions
  → TextRenderer maps the cell and glyph to pixels
  → Framebuffer performs bounded writes to the mapping
```

Construction follows the reverse ownership order: a validated `Framebuffer` is moved into a
`TextRenderer`, which is moved into a `Screen` owned alongside the parser by `Console`.
Consequently no renderer can exist without a validated mapping, parsed events cannot bypass the
cell abstraction, and higher layers cannot access the raw pointer, pitch, or pixel format.

## Concurrency and safety

All mutable console state and framebuffer writes are serialized by the endpoint's one output lock.
Input polling does not take that lock or retain scheduler state: an empty read asserts interrupts
are enabled, halts until the next interrupt, and checks `roxy-ps2` again. The framebuffer pointer
and its `Send` implementation are confined to `Framebuffer`, which is constructed only after boot
validation and memory initialization. Its safe operations enforce the validated pitch, dimensions,
and byte range before entering local unsafe blocks. The mapping is never reclaimed while the kernel
is running.
