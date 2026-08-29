---
name: jinx
description: "Use when a task involves the distro userland (the user programs and libraries built under `distro/`): viewing or patching package sources, adding a new package, or changing any packaged component's build, configuration, flags, dependencies, runtime libraries, or behavior. \"Userland\" here means the `distro/` package tree, not the kernel's user-mode execution model."
---

# Jinx — Roxy OS distro package manager

## What Jinx is

Roxy OS cross-compiles its userspace with [Jinx](https://github.com/Mintsuki/Jinx), a
shell-based meta-build-system. Each package is a **recipe** (a shell script) in `distro/`.

Throughout this skill, "userland" and "userspace" mean the `distro/` package tree — the user
programs and libraries Jinx builds — not the kernel's user-mode execution model.

**Rebuilding a dependency does NOT rebuild its dependents.** Bumping a library's
   `version`/`revision` leaves every consumer stale until you `jinx revbump <lib>` and
   `jinx update '*'`.

## Where things live

- `distro/recipes/<name>/recipe` — target userspace packages.
- `distro/host-recipes/<name>/recipe` — build-machine tools (`roxy-llvm`, `meson`).
- `distro/recipes/<name>/patches/` — optional patch set for that recipe.
- `distro/build-systems/*.sh` — shared build helpers (`meson.sh`, `autotools.sh`).
- `distro/toolchains/x86_64-roxy.cross-file` — meson cross file.
- Builds happen under `target/jinx`; the `jinx` command comes from the repo flake
  (use `nix develop` / direnv).

## Concepts every guide uses

- **recipes vs host-recipes**: `recipes/` → target sysroot; `host-recipes/` → tools for the build
  machine.
- **Dependency kinds**: `deps` (runtime), `builddeps` (build-only), `hostdeps` (host tools),
  `imagedeps` (Debian packages in the build container).

## Which guide to read

| Task | Read |
|---|---|
| Add a new package (target or host) | [`add-package.md`](add-package.md) |
| Inspect an existing package's sources | [`inspect-source.md`](inspect-source.md) |
| Patch a package | [`patch.md`](patch.md) |
| Debug a build failure | [`debug.md`](debug.md) |

Reference material:
- [`references/commands.md`](references/commands.md) — command cheatsheet.
- [`references/env-vars.md`](references/env-vars.md) — environment variables and the Jinxfile.
- [`references/recipe.md`](references/recipe.md) — recipe properties, functions, and provided
  variables.
