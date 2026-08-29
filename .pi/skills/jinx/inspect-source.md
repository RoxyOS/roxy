# Find a package's source tree

Use when you need to locate the source tree a recipe builds.

## Where a package's source lives

| What | Where |
|---|---|
| Original unpacked source — the pristine diff base; not edited | `distro/sources/<name>` (normal) or `distro/host-sources/<name>` (host) |
| Workdir — the editable working copy, and the build tree (`source_dir` points here; what `regen` diffs) | `distro/sources/<name>-workdir` |
| Patches | `distro/recipes/<name>/patches/` |

jinx's `JINX_SOURCE_DIR` is `distro/`, so sources and workdirs live under `distro/sources/`. To get
the exact path from inside the environment: `jinx run-in <name> bash` then `echo $source_dir`.