# Screenshots of the Live Roxy VM

The interactive terminal's contents are **not** on the serial log — capture the framebuffer to
see the screen. It is 1280x800 on this machine.

## Capture

```sh
<skill-dir>/scripts/hmc.sh "screendump /tmp/screen.png -f png"
file /tmp/screen.png    # expect: PNG image data, 1280 x 800, 8-bit/color RGB
```

- `png` and `ppm` are the only supported formats.
- Success = `{"return": ""}` in the reply.

## Inspect

- If the model supports images, `read` the PNG. **This session's model may not support images**;
  fall back to `serial.log` and QMP text state.
- Confirm dimensions with `file` (rarely changes) before precise mouse coordinate work.
- Use distinct names (`before.png`/`after.png`) to tell whether input changed the screen.