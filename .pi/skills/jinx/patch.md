# Patch a package

Use when a recipe's sources need a change.

## Steps

1. Get into the package's workdir (see `inspect-source.md`) and edit the sources **in place** there.
2. `jinx regen <name>` — regenerates `patches/jinx-working-patch.patch` from your workdir edits
3. `jinx rebuild <name>` — clean rebuild to verify the patched sources actually build.

The resulting `jinx-working-patch.patch` goes into `distro/recipes/<name>/`
patches/ dir; rename/commit it as the newest `NNNN-*.patch` when you're satisfied.
