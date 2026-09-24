# GDB Attach to the Running Roxy Kernel

`cargo xagent-debug` writes the GDB endpoint to the session `manifest.json`; use that port rather
than assuming `1234`. Connect the ELF matching the `profile` recorded in the manifest.

## Which ELF

The manifest records the exact kernel ELF and profile used by the VM. `dev` builds include DWARF
and source lines; `release` builds provide symbols without source-level debug information.

```sh
session=target/roxy/agent-debug/run-<id>
gdb -q "$(jq -r .kernel "$session/manifest.json")" \
  -ex "target remote $(jq -r .gdb "$session/manifest.json")"
```

## Attaching

- `target remote` pauses the VM immediately.
- The VM boots on its own; pause it right after start (QMP `stop`) to catch early boot.
- Verify the manifest's `profile` and kernel path before attaching; never mix a `dev` VM with a
  `release` ELF or another session's endpoint.

## SMP

16 vCPUs. `info threads` / `thread <id>` to switch. Only the selected thread steps; prefer
`thread apply all stop`, or `set scheduler-locking on` while stepping.

## Finishing

Do **not** leave the VM paused: send `continue` before `detach`, then stop via the PID in the
session manifest. If breakpoints don't hit, the code already ran past them (attached mid-boot);
re-pause, set breakpoints, continue — or restart and attach earlier.
