# Find a package's source tree

Use when you need to locate the source tree a recipe builds.

## Where a package's source lives

| What | Where |
|---|---|
| Sources that get built | `target/jinx/sources/<name>` (normal) or `target/jinx/host-sources/<name>` (host) |
| Workdir — the editable working copy (what `regen` diffs) | `target/jinx/sources/<name>-workdir` |
| Patches | `distro/recipes/<name>/patches/` |

To get the exact path from inside the environment: `jinx run-in <name> bash` then
`echo $source_dir`.