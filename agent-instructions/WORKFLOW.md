# Development Workflows

This document contains the repeatable procedures for changes that cross repository boundaries.
`AGENTS.md` defines the repository rules; this document explains how to apply those rules to
common development tasks. Read the applicable subsystem `DESIGN.md` before changing its design.

## mlibc development

Use the persistent `distro/sources/mlibc-workdir` tree for local mlibc iteration. The mlibc recipe
sets `clean_workdirs=no`, so this worktree is intentionally retained between builds.

Roxy maintains mlibc as a thin fork. Keep Roxy-specific work as a reviewable commit stack on top of
the selected upstream base, and upstream generally useful changes when practical. Local iteration,
publication to the canonical RoxyOS fork, and recipe integration are separate phases. Every Git
mutation in these phases still requires the explicit user confirmation mandated by
`agent-instructions/GENERAL.md`.

### Iterate locally

1. Edit and test mlibc in `distro/sources/mlibc-workdir`. Do not update the recipe commit or fetch
   remote changes during local iteration.
2. Build incrementally with:

   ```sh
   cd target/jinx
   jinx build mlibc
   ```

   Use `jinx rebuild mlibc` only when incremental state is invalid or Meson configuration must be
   recreated.
3. Rebuild the root filesystem only when userspace artifacts need testing. From the workspace
   root, remove the cached image and run:

   ```sh
   rm target/roxy/rootfs.img
   cargo run -p xtask -- rootfs
   cargo run -p xtask -- run
   ```

   Kernel-only changes should reuse the existing rootfs and must not trigger an mlibc rebuild.
4. Before publication, inspect the mlibc worktree diff and status. Exclude `.agent-lock` files,
   build output, downloaded subprojects, editor files, and unrelated upstream or Roxy changes.

### Commit and publish

1. Put one cohesive mlibc behavior or ABI consumer change in each commit. Keep its implementation,
   tests, build declarations, and mlibc-local documentation together. Do not combine independent
   sysdeps, upstream synchronization, generated files, or main Roxy OS repository changes in the
   same mlibc commit.
2. Use the subject form `roxy: <imperative summary>` for Roxy-specific commits. Preserve upstream
   commit authorship and subject when applying an upstream commit unchanged. Explain ABI choices,
   compatibility constraints, and validation in the commit body when the subject is insufficient.
3. Confirm that the intended mlibc diff builds from `mlibc-workdir`, that relevant tests pass, and
   that `git diff --check` passes. `git status --short` may contain only the intended files and the
   active collaboration lock, which must remain untracked and excluded from the commit. Record any
   unavailable or unrelated failing validation before requesting commit approval.
   Every new sysdep implementation must register its tag in the Roxy `SysdepTags` definition in
   the same commit; a compiled but undiscoverable sysdep is incomplete.
4. After explicit user approval, commit in `distro/sources/mlibc-workdir`. Do not amend, squash,
   rebase, merge, or otherwise rewrite unrelated fork history as part of a feature commit.
5. After separate explicit user approval, push the commit to the canonical RoxyOS mlibc fork. If a
   change is suitable for the mlibc project, also prepare or submit it upstream, but never make the
   Roxy recipe depend on an unmerged pull-request ref. Verify that the exact commit SHA is reachable
   from the canonical fork before changing the recipe.
6. Never force-push the canonical fork as part of this workflow. If the push is rejected or the
   remote branch moved, stop and request approval for a separate fetch and synchronization step.

### Update the recipe

1. Update `distro/recipes/mlibc/recipe` only after the exact mlibc commit is published and remotely
   reachable. Pin the immutable commit SHA; do not pin a branch, local object, or unpublished
   rewritten commit.
2. Use `0.0.0.YYYYMMDD` for the first mlibc recipe update on a date. For additional updates on the
   same date, append and increment a version suffix such as `.1`, `.2`, and so on. Keep
   `revision=1`; do not use `revision` to distinguish same-date mlibc commits. On the next date,
   return to the unsuffixed date version.
3. Perform one clean mlibc package build after updating the pin, then inspect the base dependency
   graph and rebuild affected userspace packages. Refresh the rootfs and run Roxy when installed
   userspace behavior or the kernel ABI changed. Treat the recipe update as reproducible only after
   the clean build fetches the published commit and all required integration validation succeeds.
4. Commit the recipe update in the main Roxy OS repository only after the clean build succeeds and
   after obtaining the explicit Git confirmation required by `agent-instructions/GENERAL.md`.
   Keep the compatible kernel ABI change and recipe pin in the same integration series. If
   integration exposes a defect, publish a follow-up mlibc commit instead of rewriting a commit
   already referenced by a recipe.

## Patching a distro package

Package recipes live under `distro/recipes/<package>/recipe`; host-tool recipes live under
`distro/host-recipes/<package>/recipe`. Patches belong in the owning recipe's `patches/` directory.
Jinx applies ordinary patch files with `patch -p1` in lexical filename order.

### Prepare a working patch

1. Read `distro/DESIGN.md` and the target recipe. Confirm whether the package is a normal recipe,
   a host recipe, or a `from_source` consumer. A `from_source` consumer must place patches beside
   the source-owning recipe.
2. Ensure the persistent build directory is initialized. If `target/jinx/.jinx-parameters` is
   absent, run `cd target/jinx && jinx init ../../distro`.
3. Build the package once to fetch and prepare its source, for example:

   ```sh
   cd target/jinx
   jinx build busybox
   ```

   For a host package, use the `host:<package>` selector. Jinx creates the corresponding
   `distro/sources/<package>-clean` and `distro/sources/<package>-workdir` trees.
4. Make changes only in `<package>-workdir`. Keep `<package>-clean` pristine so the generated diff
   has a stable, unmodified base. Do not edit generated build output under `target/jinx/builds/`.
5. Regenerate the working diff and rerun the prepare phase:

   ```sh
   jinx regen busybox
   # Host recipe: jinx regen host:<package>
   ```

   This writes `distro/recipes/<package>/patches/jinx-working-patch.patch`. It is a temporary
   iteration patch that Jinx applies after ordinary patches.

### Make the patch permanent

1. Review the generated diff for unrelated changes, build artifacts, absolute paths, and accidental
   edits to the clean tree. Keep the patch in unified-diff form with paths compatible with `-p1`.
2. Rename the working file to an ordered, descriptive name such as
   `0001-roxy-disable-unsupported-feature.patch`. Delete the old
   `jinx-working-patch.patch`; do not leave both copies in the recipe.
3. Bump the recipe `revision` when the package input changes without changing its upstream
   version. Keep the patch focused on the package's Roxy compatibility requirement; put dependency,
   source-pin, or build-system changes in the recipe instead of hiding them in a patch.
4. Rebuild the package, inspect the affected dependency graph, and rebuild any required
   dependents from the persistent Jinx directory:

   ```sh
   cd target/jinx
   jinx rebuild <package>
   jinx dry-run base
   # If the package is in the base dependency graph:
   jinx build base
   ```

   Use `host:<package>` for host recipes; host-only changes do not require rebuilding `base`.
   If the patch changes an installed userspace artifact, refresh the rootfs with the mlibc/rootfs
   procedure above and run `cargo run -p xtask -- run`.
5. Before handoff, verify that the patch applies to a pristine source in a fresh or cleaned Jinx
   build state, then run the package's available checks and `git diff --check`. Do not treat a
   successful incremental build alone as proof that the committed patch is reproducible.

## Final validation

- Kernel or Rust changes: run the strongest stage-appropriate checks described in `AGENTS.md`;
  `cargo run -p xtask -- check` is the repository task-runner entry point when applicable.
- Package-only changes: rebuild the affected package and its required dependents with Jinx; refresh
  the rootfs only when the installed userspace needs verification.
- ABI changes: validate the kernel and mlibc contract together, including generated/layout checks
  required by the active stage.
- Always inspect `git diff --check`, `git status --short`, recipe revisions, patch ordering, and
  generated-artifact exclusions before completing the change.
