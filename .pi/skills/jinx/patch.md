# Patch a package

Use when a recipe's sources need a change.

## Steps

1. Edit the package's working copy at `distro/sources/<name>-workdir` **in place**. This is the
   edit tree `jinx regen` diffs against `-clean`. Workdir edits do not apply automatically: an
   edit only reaches a build after `jinx regen` (step 2) regenerates `jinx-working-patch.patch`,
   which is then applied at build time. Never edit `distro/sources/<name>` or the `-clean`
   snapshot: they are only the pristine diff base, and edits there are discarded or poison the
   generated patch. Never hand-write or directly edit a package patch — always generate it from
   the `-workdir` edits via `jinx regen`.
2. `jinx regen <name>` — regenerates `patches/jinx-working-patch.patch` from your workdir edits.
   This is what makes the edit apply: without it, a workdir-only change never reaches a build.
3. `jinx rebuild <name>` — clean rebuild to verify the patched sources actually build.

`jinx-working-patch.patch` is a temporary iteration patch, applied after the ordinary patches.
When satisfied: rename it to an ordered, descriptive name like `0001-fix-thing.patch` (lexical
order = apply order, `patch -p1`), delete the working copy, and bump the recipe's `revision`
(input changed without a version bump).
