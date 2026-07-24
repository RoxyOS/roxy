# Development Workflows

This document contains the repeatable procedures for changes that cross repository boundaries.
`AGENTS.md` defines the repository rules; this document explains how to apply those rules to
common development tasks. Read the applicable subsystem `DESIGN.md` before changing its design.

## mlibc development

Use the persistent `distro/sources/mlibc-workdir` tree for local mlibc iteration. The mlibc recipe
sets `clean_workdirs=no`, so this worktree is intentionally retained between builds.

By default, limit mlibc work to local files in that worktree. Do not fetch, pull, rebase, switch,
or otherwise synchronize with upstream mlibc or RoxyOS fork remotes. Do not create mlibc commits
or push mlibc changes on the user's behalf; hand off the tested local changes so the user can
commit and push them.

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
4. Once behavior is stable, report the local diff and validation performed, then leave commit and
   push to the user. Do not update the recipe to an unpublished local commit.
5. Only after the user supplies a commit already pushed to the RoxyOS fork and explicitly requests
   integration, update `distro/recipes/mlibc/recipe` with that commit and its `version`/`revision`,
   then perform one clean package and rootfs build. Treat the result as reproducible only after
   that clean build succeeds. Use `0.0.0.YYYYMMDD` for the first mlibc recipe update on a date.
   For additional updates on the same date, append and increment a version suffix such as `.1`,
   `.2`, and so on. Keep `revision=1` for these updates; do not use `revision` to distinguish
   same-date mlibc commits. On the next date, return to the unsuffixed date version.

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
