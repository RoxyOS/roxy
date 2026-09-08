# Screenshots of the Live Roxy VM

Capture the current framebuffer while the VM runs and inspect it. This is the only reliable way
to *see* what the framebuffer shell or a GUI app is showing, because the interactive terminal's
contents are not on the serial log.

The framebuffer on this machine is **1280x800** (verified from a live dump). Use that for pointer
coordinate conversion in `mouse.md`.

## Capture a PNG

There is no QMP `screendump` command; route it through the HMP layer via
`human-monitor-command`:

```sh
SO=/home/elysia/projects/roxy-os/target/roxy/agent-debug/qmp.sock
OUT=/home/elysia/projects/roxy-os/target/roxy/agent-debug/screen.png
timeout 5 bash -c "{ printf '{\"execute\":\"qmp_capabilities\"}\\n'; sleep 0.1; \
      printf '{\"execute\":\"human-monitor-command\",\"arguments\":{\\\"command-line\\\":\\\"screendump $OUT -f png\\\"}}\\n'; \
      sleep 0.5; } | nc -U $SO" >/dev/null
file "$OUT"     # expect: PNG image data, 1280 x 800, 8-bit/color RGB
```

`-f png` is supported (verified: only `png` and `ppm` are accepted formats). Writing PNG keeps the
file small; `ppm` is larger but trivially parseable if you need raw pixel data.

Success is signaled by `human-monitor-command` returning `{"return": ""}`. A failed screendump
(e.g. before a mode is set) returns an error JSON and no file is produced.

## Inspect the image

- If the current agent model supports images, read the PNG directly with the `read` tool and
  describe what is on screen. **The agent model in this session may not support images**; in that
  case rely on `serial.log` and QMP/HMP textual state instead (see `SKILL.md`), and treat the
  dump as evidence you can hand to a human or a vision-capable step.
- To read the framebuffer dimensions off any dump even without a viewer: `file screen.png`.
- Compare two dumps by filename (screen1/screen2) if you are just checking whether output changed.

## Coordinate check for the pointer

The framebuffer is a fixed device buffer; its pixel size is the same value you apply to mouse
coordinate mapping. Read `file` on a fresh dump to confirm the current mode before doing precise
`mouse.md` work — resolution changes are rare here (stays 1280x800), but confirm rather than
assume.

## Tips

- Give every dump a distinct name per observation point (e.g. `before.png`/`after.png`) so you can
  tell whether input had an effect.
- Timestamp the filename if you are sampling over time: `screen-$(date +%s).png`.
- Run the capture under `timeout`; if the guest is paused it still answers, but a wedged QEMU
  would otherwise hang the shell.