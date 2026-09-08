# Mouse Input into the Live Roxy VM

The guest's pointer is a **PS/2 mouse** (`info mice` reports `QEMU PS/2 Mouse`). PS/2 mice report
**relative** motion only — there is no absolute pointer position on the guest, so both QEMU
injectors express movement as deltas, and precise "click at pixel (x, y)" requires you to track the
cursor yourself. The privilege is that input flows straight to the guest's 8042 driver.

## Preferred: QMP `input-send-event`

Use the same `input-send-event` command as keyboards, with `type`:

- **relative motion**:
  ```json
  {"type":"relative","data":{"axis":0,"value":10}}   {"type":"relative","data":{"axis":1,"value":-6}}
  ```
  axis `0` = X (right +), axis `1` = Y (down +).
- **button press/release**:
  ```json
  {"type":"button","data":{"button":0,"down":true}}    {"type":"button","data":{"button":0,"down":false}}
  ```
  button `0` = left, `1` = right, `2` = middle.

Each pointer action is a separate event in the `events` array; you can send a whole gesture
(multiple moves then a click) in one `input-send-event`:

```sh
SO=.../target/roxy/agent-debug/qmp.sock
timeout 5 bash -c "{ printf '{\"execute\":\"qmp_capabilities\"}\\n'; sleep 0.1; \
      printf '{\"execute\":\"input-send-event\",\"arguments\":{\\\"events\\\":[\\\
        {\\\"type\\\":\\\"button\\\",\\\"data\\\":{\\\"button\\\":0,\\\"down\\\":true}},\\\
        {\\\"type\\\":\\\"button\\\",\\\"data\\\":{\\\"button\\\":0,\\\"down\\\":false}}]}}\\n'; \
      sleep 0.2; } | nc -U $SO"
```

## Alternative: HMP `mouse_move` / `mouse_button`

The HMP equivalents are simpler for occasional human use (verbatim from `help`):

- `mouse_move dx dy [dz]` — relative move, signed deltas.
- `mouse_button state` — button state **mask**: `1`=left, `2`=right, `4`=middle held; `0` = all
  released. To click left: send `mouse_button 1` then `mouse_button 0`.

Wrap them in `human-monitor-command` to avoid raw-HMP echo noise, exactly as in `keyboard.md`.

## Clicking a screen coordinate

Because motion is relative, you must track accumulated position. Given a target framebuffer pixel
(x, y) and framebuffer size from `screenshot.md` (default 1280x800):

1. Decide units: PS/2 deltas are linear-ish but not a guaranteed 1:1 pixel map. Use small deltas
   (e.g. ±8) and verify by screenshot (`screenshot.md`) that the cursor actually moved; scale up
   if it lagged.
2. Emit relative moves in `dx`/`dy` steps toward the target from the last known position.
3. Click and re-capture to confirm.

For a GUI that lifts the mouse into an absolute virtual coord space (e.g. a window manager
switching to a tablet), relative moves become unreliable; in that case prefer a second
screendump to re-base the cursor each step.

## Verifying

- `mouse_move`/`mouse_button`/`input-send-event` returning `{"return": ...}` confirms QEMU
  accepted the injection, **not** that the kernel saw it.
- Confirm by screenshot and/or by watching `serial.log` if the app or driver prints pointer
  events. Many Roxy GUI apps have no serial logging, so screenshot is the ground truth.

## Notes

- Always run under `timeout`; a paused or wedged VM would otherwise hang the shell.
- For keyboard *and* mouse together, interleave their `input-send-event` events in one stream;
  each carries its own `type`, so they compose fine.