# Xtask Design

## Purpose and scope

`xtask` is the repository's developer-task entry point. It orchestrates rootfs creation, kernel
builds, boot-image/run commands, kernel-test execution, and stage-appropriate checks. It does not
contain kernel runtime behavior or duplicate the build logic owned by external tools.

## Command flows

- `rootfs` builds and installs the Jinx `base` package into a clean staging tree, then creates the
  ext4 root image.
- `image` builds rootfs and release kernel, then creates a bootable image.
- `run` builds the same inputs and launches the configured emulator path.
- `test` builds the kernel-test image and runs the test harness.
- `check` runs formatting, workspace checks, Clippy, release kernel checks, test-kernel checks, and
  diff whitespace validation.

## Ownership and external boundaries

Artifacts live below `target/roxy` or the target-specific Cargo output tree. Jinx owns package
resolution and staging installation; Cargo owns Rust builds; filesystem/image tools own image
formatting; the task runner supplies sequencing, paths, and contextual errors.

Commands must use the workspace root derived from `CARGO_MANIFEST_DIR` and must not depend on the
caller's current directory. A failed external command aborts the current task instead of silently
using stale output.

## Limits

The task runner assumes the repository's current x86_64 target, Jinx layout, ext4 rootfs format,
and installed host tools. It is not a general build system or distribution package manager.
