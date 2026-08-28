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

The base image provides Vim as its interactive editor. Vim uses the `tiny` feature set and disables
GUI, X11, localization, channels, and optional system integrations, while retaining the upstream
runtime installed by its Autotools flow. Its terminal dependency is the shared wide-character
ncurses library; Roxy enables ncurses' ordinary ELF shared-library rules with a package patch.

Meson identifies the host system as Roxy. Autotools packages use the `x86_64-unknown-roxy-mlibc`
host tuple: `toolchains/config.sub` accepts the triplet, and `autotools_patch_roxy_target` swaps
that config.sub into upstream build trees and folds `roxy-mlibc` into libtool's linux-family
branch where the shipped configure lacks a native `*-mlibc` one, so shared libraries can be built.
The compiler's `x86_64-unknown-roxy` target remains authoritative for generated code and linking.

## X11 userspace stack

`base` installs the X server (`xorg-server`), the twm window manager, and the xeyes demo client;
the X packages form a layered graph. Protocol headers (`xorgproto`, `xtrans`) sit below the XCB
transport (`xcb-proto`, `libxau`, `pthread-stubs`, `libxcb`) and the Xlib family (`libx11`, `libxext`,
`libxfixes`, `libice`, `libsm`, `libxt`, `libxi`, `libxmu`, `libxkbfile`). Font plumbing (`zlib`, `libfontenc`,
`libxfont2`, `font-util`, `font-misc-misc`) and XKB data (`xkbcomp`, `xkeyboard-config`) feed the
server and driver layers (`xorg-server`, `xf86-video-fbdev`, `twm`, `xeyes`); `pixman`, `libxcvt`, and
`libmd` (server SHA1) complete the set. Release tarballs ship pre-generated `configure`, so `util-macros`
is not required; `font-util` still supplies the encoding map files and `fontutil.pc`.

Every X configure script and Meson `dependency()` call resolves libraries through pkg-config, which
no earlier package used. The autotools adapter exports `PKG_CONFIG_LIBDIR` (the sysroot pkg-config
directories) and `PKG_CONFIG_SYSROOT_DIR` (`/sysroot`); for Meson builds the cross file's `sys_root`
and `pkg_config_libdir` properties feed the same variables. Cross-compiled packages therefore see
only target `.pc` files, and `pkg-config --variable` output stays unprefixed, keeping compile-time
paths such as `XKB_BASE_DIRECTORY=/usr/share/X11/xkb` equal to the runtime paths.

The X server builds with the `x86_64-unknown-roxy-mlibc` host tuple. Configure recognizes no such
OS and selects the `os-support/stub` layer (no-op VT, IO-port, and PM hooks), so no Linux OS code
is needed. GLX,
glamor, DRI, DRM, pciaccess, and MIT-SHM are explicitly disabled; SHA1 comes from `libmd`. The font
encoding maps are resolved by `ucs2any` through `fontutil.pc`'s `mapdir`, which records the runtime
(host) path; `font-misc-misc` binds that host path to the sysroot copy during its build and keeps
only the ISO8859-1 encoding for the default "fixed" font. Host-side font tooling (`bdftopcf`,
`mkfontdir`, `mkfontscale`, `bdftruncate`, `ucs2any`) comes from the Debian `xfonts-utils` image
dependency rather than host recipes.

Known runtime gaps are tracked separately from the recipe graph: the kernel currently lacks
`pipe(2)` and addressed Unix sockets (blocking xkbcomp invocation and X client connections), and
the `fbdevhw` module needs a `linux/fb.h` header in the mlibc sysroot before `xf86-video-fbdev` can
drive `/dev/fb0`.

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
Roxy's terminal sysdeps route `tcgetattr`, `tcsetattr`, `tcgetwinsize`, and `tcsetwinsize` through
the shared ioctl syscall using the Linux-compatible request numbers mirrored by the kernel.
An ABI change is incomplete until the relevant mlibc source pin/recipe and rootfs package graph can
build together. Failed source verification, configure, build, or install steps must abort rather
than falling back to stale artifacts.

## Limits

The current distribution is a minimal x86_64 Roxy userspace centered on mlibc, Bash, and a
purposefully small BusyBox configuration. BusyBox supplies a shell and basic file and process
utilities; Linux-specific mount-table, filesystem-statistics, and UTMP features remain disabled
until their kernel and mlibc contracts are supported. Clang userspace builds use the
`x86_64-unknown-roxy` target; the kernel remains a separate
`x86_64-unknown-none` Rust target. This is not a general package repository and does not define
runtime service management, upgrades, or binary package compatibility across kernel ABI revisions.
