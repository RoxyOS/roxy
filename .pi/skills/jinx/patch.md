# Patch a package

Use when a recipe's sources need a change.

## Steps

1. Get into the package's workdir (see `inspect-source.md`) and edit the sources **in place** there.
   Never hand-write or directly edit a package patch — always generate it from workdir edits.
2. `jinx regen <name>` — regenerates `patches/jinx-working-patch.patch` from your workdir edits
3. `jinx rebuild <name>` — clean rebuild to verify the patched sources actually build.

`jinx-working-patch.patch` is a temporary iteration patch, applied after the ordinary patches.
When satisfied: rename it to an ordered, descriptive name like `0001-fix-thing.patch` (lexical
order = apply order, `patch -p1`), delete the working copy, and bump the recipe's `revision`
(input changed without a version bump).
