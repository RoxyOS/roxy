# Mouse Input into the Live Roxy VM

The guest pointer is a **PS/2 mouse**, so motion is **relative** only — no absolute pointer
position. Track the cursor yourself; screenshot to verify after moving.

## Preferred: QMP `input-send-event`

- Relative motion — axis is `"x"` or `"y"`:
  `{"type":"rel","data":{"axis":"x","value":20}}` (right/down positive).
- Button — `"left"`/`"right"`/`"middle"`, with `down` true/false:
  `{"type":"btn","data":{"button":"left","down":true}}`

A whole gesture (moves + click) fits in one `input-send-event`:

```sh
<skill-dir>/scripts/qmp.sh '{"execute":"input-send-event","arguments":{"events":[{"type":"rel","data":{"axis":"x","value":20}},{"type":"rel","data":{"axis":"y","value":10}},{"type":"btn","data":{"button":"left","down":true}},{"type":"btn","data":{"button":"left","down":false}}]}}'
```

## Alternative: HMP

- `<skill-dir>/scripts/hmc.sh 'mouse_move dx dy'` — relative move, signed deltas.
- `<skill-dir>/scripts/hmc.sh 'mouse_button 1'` then `'mouse_button 0'` — mask `1`=L, `2`=R, `4`=M;
  `0`=released.

## Clicking a screen coordinate

Accumulate position against the target pixel (from `screenshot.md`, default 1280x800). PS/2
deltas are not guaranteed 1:1 pixels: use small deltas (±8), screenshot to confirm the cursor
moved, scale up if it lagged. For a GUI using absolute/tablet mode, re-base with a fresh
screenshot each step.

## Verifying

QMP's return only proves QEMU accepted the events, not that the guest saw them. Screenshot is
ground truth; most Roxy GUI apps have no serial logging.