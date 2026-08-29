# Jinx Command reference

## Commands

| Command | What it does |
|---|---|
| `jinx help` | Usage. |
| `jinx init <dir> [KEY=VALUE]…` | Create a build dir; sets `JINX_ARCH` (default `uname -m`), records source dir. |
| `jinx update [-b] [pkgs]` | Rebuild stale XBPS files; `-b` also builds never-built. `update '*'` default. |
| `jinx build [pkgs]` | Incremental; re-runs `build()`+`package()`, skips `configure()` if build dir exists. |
| `jinx rebuild [pkgs]` | Delete build dir → fresh `configure()`. Required after source/recipe/version changes. |
| `jinx revbump [pkgs]` | Bump `revision` on all transitive dependents (not the target itself). |
| `jinx regenerate\|regen [pkg]` | Regenerate `jinx-working-patch.patch` from in-place workdir edits; re-runs `prepare()`. |
| `jinx install [-f] <sysroot> [pkgs]` | Install into sysroot; detects conflicts. `-f` reinstalls. |
| `jinx dry-run [pkgs]` | Print topological build order that would build. |
| `jinx download [pkgs]` | Fetch pre-built XBPS from `JINX_REPO_URL`. |
| `jinx run-in <pkg> <cmd>…` | Run a command in the recipe's prepared container (network on). |
| `jinx rebuild-cache` | Rebuild `.jinx-cache/` (Debian image, XBPS, debootstrap). |

Prefix a package with `host:` to target `host-recipes/` (e.g. `jinx build host:roxy-llvm`); globs
work (`'*'`, `'host:*'`).

## `jinx regen` behavior

- It diffs the workdir against the source tree **with the official patches already applied**, so
  it emits only the increment since the last regen — not a full diff against pristine upstream.
- It syncs the workdir changes into `distro/sources/<name>-clean`, so the `-clean` snapshot is
  **no longer pristine** once a regen has run. Never assume `-clean` equals the upstream tarball.
- During a build, patches apply in lexical order (`0001-*`, `0002-*`, ...) followed by
  `jinx-working-patch.patch`. The working patch carries the un-published increment; keep it until
  the change is folded into an official patch, then delete it.