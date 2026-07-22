# `fbterm` Design

## Purpose and scope

`roxy-fbterm` provides the framebuffer-backed implementation of the shared terminal endpoint.
It owns text rendering state and framebuffer writes and adapts the PS/2 driver's input stream; it
does not own boot protocol parsing, process descriptors, keyboard hardware, terminal line
discipline, or kernel diagnostics.

## Ownership and initialization

In normal builds, core initializes `fbterm` after boot metadata and kernel memory are ready. Kernel
test builds select serial directly and do not require framebuffer initialization. The endpoint uses
the first Limine framebuffer and its HHDM virtual address for the kernel lifetime. Unsupported modes
are reported to core, which selects the serial terminal instead. Each open file retains an `Arc` to
the same synchronized endpoint. A successful initialization publishes that `Arc` exactly once; a
mode-validation failure leaves the global endpoint uninitialized, while a second successful
initialization attempt violates the core startup contract and panics.

## Terminal behavior

The endpoint renders printable ASCII with the regular Terminus 8x16 bitmap font from
`embedded-bitmap-fonts`. Only that crate's `terminus` feature is enabled. It is `no_std`, uses the
Apache-2.0 license, and provides a fixed bitmap whose dimensions match the cell renderer without
font parsing, allocation, or scaling. LF advances a row, CR returns to column zero, backspace
clears the preceding cell, and tab advances to the next eight-column stop. Reaching the bottom
scrolls the framebuffer by one glyph row. Input is supplied by the PS/2 endpoint described below;
the framebuffer console itself only owns output rendering.

Only Limine RGB 32-bit modes are accepted. Color masks determine packed foreground/background
pixels. Reads delegate directly to `roxy-ps2` and return its raw ASCII byte stream. The endpoint
polls until at least one byte is available, halting the CPU between empty checks so IRQs can make
progress. It does not echo, buffer lines, translate newlines, or hold the console lock while
waiting. Full
ANSI parsing, Unicode, alternate input devices, PTYs, and diagnostic mirroring are outside the
current contract.

## Rendering model

Rendering is split into three ownership layers:

```text
Console → TextRenderer → Framebuffer → framebuffer mapping
```

`Framebuffer` is the pixel-addressed hardware boundary. It validates and owns the boot-provided
mapping, physical pixel dimensions, pitch, and RGB channel layout. It converts RGB components into
native pixels and provides bounded pixel, rectangle, and pixel-row operations. It has no concept of
characters, cells, cursors, or terminal control bytes.

`TextRenderer` interprets the framebuffer as a grid of fixed 8x16 cells. It derives and owns the
grid's total `columns` and `rows` from the framebuffer dimensions; a partial cell at the right or
bottom edge is outside the text grid. The renderer owns the foreground and background colors and
adapts Terminus's binary glyph output to pixels inside one selected cell. The adapter remains
private to the rendering layer, so neither the framebuffer boundary nor the terminal state machine
depends on third-party graphics types. The renderer can draw or clear a cell, reversibly invert a
selected cell for cursor display, and scroll the complete text region upward by one cell row. It
does not own a current cell, advance a cursor, or interpret control bytes.

`Console` is the byte-level terminal state machine. It owns only the current cursor `column` and
`row`, while querying `TextRenderer` for the grid bounds. Printable ASCII draws into the current
cell and advances the cursor. The current cell is shown as a reversible full-cell inverse cursor
while the console is idle; batched writes hide it before processing and restore it at the final
position. Reaching the final column wraps to the next row. LF selects column zero of the next row,
CR selects column zero without changing rows, backspace moves left and clears that cell when
possible, and tab emits spaces until the next eight-column stop. Moving beyond the last row asks
`TextRenderer` to scroll one cell row and leaves the cursor on the new final row. Cursor inversion
uses the renderer's foreground/background XOR mask, so moving away from a cell restores its glyph
instead of clearing it. Ignored bytes neither draw nor move the cursor.

The output path is therefore:

```text
input byte
  → Console interprets terminal semantics and selects a cell
  → TextRenderer maps the cell and glyph to pixels
  → Framebuffer performs bounded writes to the mapping
```

Construction follows the reverse ownership order: a validated `Framebuffer` is moved into a
`TextRenderer`, which is moved into a `Console`. Consequently no renderer can exist without a
validated mapping, no console can bypass the cell abstraction, and higher layers cannot access the
raw pointer, pitch, or pixel format.

## Concurrency and safety

All mutable console state and framebuffer writes are serialized by the endpoint's one output lock.
Input polling does not take that lock or retain scheduler state: an empty read asserts interrupts
are enabled, halts until the next interrupt, and checks `roxy-ps2` again. The framebuffer pointer
and its `Send` implementation are confined to `Framebuffer`, which is constructed only after boot
validation and memory initialization. Its safe operations enforce the validated pitch, dimensions,
and byte range before entering local unsafe blocks. The mapping is never reclaimed while the kernel
is running.
