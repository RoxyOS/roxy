# Xtask Design

## Purpose and scope

`xtask` is the repository's developer-task entry point. It orchestrates rootfs creation, kernel
builds, boot-image/run commands, kernel-test execution, and stage-appropriate checks. It does not
contain kernel runtime behavior or duplicate the build logic owned by external tools.

## Command flows

All commands accept a `--arch` flag (`x86_64` default, or `aarch64`) that selects the kernel
Rust target triple and the architecture suffix on `rootfs` and ISO artifact names. `x86_64` is the
only fully runnable backend today; `aarch64` resolves the target triple, artifact names, and QEMU
machine, but its boot (firmware/EFI) and userspace toolchain are not yet wired, so aarch64 builds
top out where those dependencies are missing.

- `rootfs` builds and installs the Jinx `base` package into a clean staging tree, then creates the
  ext4 root image, replacing any existing image.
- `image` reuses an existing rootfs image or builds one when absent, builds the release kernel, then
  creates a bootable image.
- `run` uses the same cached-rootfs behavior and launches QEMU with its default graphical display
  so framebuffer output is visible while serial remains attached to the invoking terminal.
- `test` uses the same cached-rootfs behavior, builds the kernel-test image, and runs the harness.
- `check` runs formatting, workspace checks, Clippy, release kernel checks, test-kernel checks, and
  diff whitespace validation without building userspace or rootfs artifacts.

Artifact names carry the architecture suffix because the root filesystem contains that
architecture's cross-compiled userspace binaries and the ISO embeds them, so per-arch content must
not share a single name or a shared cache entry:

## Ownership and external boundaries

Artifacts live below `target/roxy` or the target-specific Cargo output tree. Jinx owns package
resolution and staging installation; Cargo owns Rust builds; filesystem/image tools own image
formatting; the task runner supplies sequencing, paths, and contextual errors.

Commands reuse `target/roxy/rootfs-{arch}.img` only when it contains the ext4 superblock magic.
Missing, truncated, or interrupted outputs are rebuilt instead of being treated as valid caches.
The `{arch}` suffix is part of cache identity, so an x86_64 rootfs is never mistaken for an
aarch64 one. Distribution or package changes do not invalidate a structurally valid image
automatically;
developers explicitly run `rootfs` when they need a fresh userspace image. Removing the cached
image also causes the next consumer command to rebuild it.

Commands must use the workspace root derived from `CARGO_MANIFEST_DIR` and must not depend on the
caller's current directory. A failed external command aborts the current task instead of silently
using stale output.

## Limits

The task runner assumes the repository's current x86_64 backend, Jinx layout, ext4 rootfs format,
and installed host tools. It is not a general build system or distribution package manager.
`aarch64` plumbing (triple, artifact names, QEMU machine) is present, but the aarch64 kernel
backend, boot path, and userspace toolchain are not; until they land, `--arch aarch64` fails at the
first x86_64-only dependency it reaches instead of silently emitting a wrong build.
