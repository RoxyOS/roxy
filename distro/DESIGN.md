# Distribution Build Design

## Purpose and scope

`distro` defines the reproducible userspace package graph and cross-build configuration consumed by
Jinx. It owns package recipes, source pins, shared build-system adapters, and the Roxy target
toolchain description. It does not build the kernel or create the final root filesystem image;
`xtask` orchestrates those steps.

## Package and source model

`Jinxfile` pins the supported Jinx version and Debian snapshot. Each recipe declares its source,
integrity or commit pin, build/image dependencies, cross-compilation mode, and package phases. The
`base` metapackage defines the minimal userspace installed into the rootfs staging tree.

Shared autotools and Meson scripts own common cross-build mechanics. They invoke the Roxy Clang
driver directly; the driver owns target defaults, sysroot search paths, linker selection, CRT files,
and runtime libraries. Package recipes should contain only package-specific configuration and must
not duplicate target setup already supplied by those adapters.

The `roxy-llvm` host package pins and builds the Roxy LLVM fork as a native toolchain containing
Clang, LLD, the required LLVM binary utilities, resource headers, and Roxy compiler-rt builtins.
Cross-compiled packages depend on this host package instead of distribution LLVM packages so the
compiler driver and runtime always describe the same target contract.
The package exposes LLD through both `ld.lld` and the GNU-compatible `ld` entry point so compiler
tool discovery used by Autotools remains self-contained within the host package.
The target contract includes ELF IFUNC because x86_64 mlibc uses it for selected routines and its
runtime linker resolves the resulting relocations.
Roxy mlibc explicitly enables its glibc extension option to provide the compatibility interfaces
expected by GNU userspace, including the `sys/ioctl.h` contract used by Bash job control.
Packages that execute native build generators must separately declare a Linux host development
environment; `roxy-llvm` intentionally does not bundle distribution C headers or host CRT objects.

Meson identifies the host system as Roxy. Autotools packages temporarily use the compatible
`x86_64-unknown-none` host tuple because GNU `config.sub` rejects the Roxy OS name; the compiler's
`x86_64-unknown-roxy` target remains authoritative for generated code and linking.

## Build flow

```text
Jinx initialize → resolve pinned sources → prepare source dependencies
→ cross-configure with Roxy toolchain → build → install package tree
→ xtask installs base into staging → create ext4 rootfs image
```

Source-network access must be declared explicitly. Patches and source metadata are versioned inputs;
generated build directories and installed staging trees remain build artifacts outside this design.

## ABI and failure contracts

Userspace packages must target the syscall and ABI exposed by the kernel and Roxy mlibc sysdeps.
An ABI change is incomplete until the relevant mlibc source pin/recipe and rootfs package graph can
build together. Failed source verification, configure, build, or install steps must abort rather
than falling back to stale artifacts.

## Limits

The current distribution is a minimal x86_64 Roxy userspace centered on mlibc and Bash. Clang
userspace builds use the `x86_64-unknown-roxy` target; the kernel remains a separate
`x86_64-unknown-none` Rust target. This is not a general package repository and does not define
runtime service management, upgrades, or binary package compatibility across kernel ABI revisions.
