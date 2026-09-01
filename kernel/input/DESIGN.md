# Input Design

## Purpose and scope

`roxy-input` defines the shared raw-input boundary and the **input-manager** layer between
keyboard drivers and consumers. Drivers own hardware access and scancode parsing; the input
manager owns listener registration and event broadcast. It owns no hardware, queues, terminal
semantics, file descriptors, or output devices.

## Contract and limits

`roxy_input::InputManager` is a process-wide singleton that broadcasts `KeyEvent`s to every
registered `InputListener`. A driver calls `roxy_input::publish(key)` from its IRQ handler after
parsing one scancode byte; the manager iterates its listener list and delivers one copy of the
event to each. This is a push model: every listener receives the event in the producer's context
(IRQ) and is responsible for buffering or dropping it if it cannot be processed immediately.

`KeyEvent` carries a layout-neutral `KeyCode` (the single source of truth for key identity) and a
pressed/released `KeyState`. Layout mapping, byte encoding, buffer filling, signal generation,
echo, canonical processing, terminal attributes, blocking APIs, and device enumeration are
absent from this boundary; they belong to consumers such as the TTY.

## InputManager

The `InputManager` holds a `Lock<Vec<Weak<dyn InputListener>>>`. Registration happens once per
consumer at boot, before interrupts are enabled, and stores a weak reference so that a dropped
consumer is automatically unregistered on the next `publish` traversal.

`publish` runs in the driver's IRQ context. It acquires the listener list lock, iterates live
listeners, and calls each listener's `on_recive_input(key)`. ZST `()` errors from full queues or
failed `try_lock` calls are absorbed by the listener. A listener that returns an error is not
unregistered; only a dropped `Arc` (detected via weak reference upgrade failure) triggers
cleanup.

## KeyEvent, KeyCode, KeyState

`KeyCode` names physical keys in a US 104-key layout and is the single source of truth for key
identity. Consumers map it to characters, escape sequences, or ABI-specific codes according to
their own layout engine (the TTY through `pc_keyboard`, a graphics stack through its own layout
engine). `KeyState` distinguishes presses from releases. `KeyEvent` pairs one code with one
state. The type is `Clone`, `Copy`, `Debug`, and `PartialEq` for testing convenience.

## Driver contract

A keyboard driver:
1. receives hardware scancode bytes in its IRQ handler,
2. parses the byte into a `KeyEvent` (pressed or released, including modifiers and
   character-key releases),
3. calls `roxy_input::publish(key)` exactly once per parsed event.

The driver does not buffer events, notify consumers, register listeners, or keep a queue.
Back-pressure (a consumer that cannot accept a new event) is handled by the consumer's own
bounded queue; the driver simply drops the event if the consumer's queue is full.

## Consumer contract

A consumer (TTY, evdev, …):
1. creates an `Arc<dyn InputListener>` whose `on_recive_input` receives each `KeyEvent`,
2. registers it with `roxy_input::register_listener(listener)` at boot,
3. buffers the event or processes it inline (including with `try_lock` in IRQ context).

The consumer owns its own bounded queue and decides whether to process immediately (IRQ-path
for signal responsiveness) or defer to a read/wait path.

## Layout

Layout mapping is deliberately absent from this boundary. The TTY owns a `pc_keyboard`
`EventDecoder<Us104Key>` for its layout; a graphics stack would own its own layout engine.
No layout information, modifier state, or character encoding crosses the
driver–manager–consumer boundary.