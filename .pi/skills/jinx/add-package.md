# Add / wire in a new package

Use when a package needs to be built into the Roxy OS userspace (or a new build tool added to the
host).

## Decision: `recipes/` vs `host-recipes/`

- Goes into the target sysroot (libc, utilities, shared libs) → **`recipes/<name>/`**.
- Is a build-machine tool the target recipes consume (cross-compiler, code generator, build system)
  → **`host-recipes/<name>/`** (e.g. `meson`, `roxy-llvm`).
- Is it pulled in as a `hostdeps` by recipes? Then it must be a host recipe.

## Decide the source strategy

Roxy OS recipes use one of three source forms. Pick from the current `distro/recipes/`:

- **Tarball** (most common, e.g. `meson`, `vim`): set `tarball_url` + `tarball_sha256`.
  Set `tarball_sha256="?"` and let a first build auto-fill the checksum, then pin it.
- **Git clone**: set `git_url` + `commit` (full 40-hex, no branches/tags) +
  `shallow=no` when history is needed. This is the Roxy default for own/dependent repos.
- **Local dir** (`source_dir`): build code living inside the repo. Files must stay inside the dir.

## Recipe template

```bash
#!/usr/bin/env bash

version=…          # package upstream version
revision=1         # bump when config/logic changes without a version bump

# source — pick ONE of:
tarball_url="https://…/${version}.tar.gz"
tarball_sha256="…"
#   or
git_url="https://github.com/RoxyOS/<name>"
commit="<40-hex>"
shallow=no

cross_compile=yes                   # needed for anything cross-built for x86_64-unknown-roxy
deps="…"                            # target runtime deps
builddeps="…"                       # build-only target deps, not packed
hostdeps="roxy-llvm"                # host tools this build needs
imagedeps="…"                       # Debian packages in the build container
clean_workdirs=no                   # keep workdir for iteration (Roxy convention for slow builds)

source "${base_dir}/build-systems/meson.sh"    # or autotools.sh, etc.

prepare() { … }        # optional; runs from source_dir after patches
configure() { meson_configure / autotools_configure … }
build()     { meson_build / autotools_build … }
package()   { meson_install / autotools_install … }
```

For meson projects use the helpers from `build-systems/meson.sh`; for autotools use `build-systems/autotools.sh`, etc.

After writing the recipe, do `jinx build <name>` to test the recipe.

## Related

- [`references/recipe.md`](../references/recipe.md) — full property list.
