# Recipe property & variable reference

Reference for recipe files in `distro/recipes/` and `distro/host-recipes/`.

## Recipe properties

| Property | Meaning |
|---|---|
| `version` (req) | Upstream version; part of the XBPS filename. Bump on real version changes. |
| `revision` (req) | Bump when build logic/patches change without a version change. `revbump` edits this. |
| `source_dir` | Build code living inside the repo (local package). Files must stay in the dir. |
| `from_source` / `from_host_source` | Pull sources from another recipe. Mutually exclusive. (Not used currently.) |
| `tarball_url` | Download source tarball; needs a checksum. |
| `tarball_sha256` / `tarball_sha512` / `tarball_blake2b` | Verify the tarball. Set to `"?"` to auto-fill on first download. |
| `git_url` | Clone a repo; requires `commit` (full 40-hex). |
| `commit` | Pinned commit (full hash; no branches/tags) — reproducibility. |
| `shallow` | `yes`/`no` (default `yes`); `no` keeps history (e.g. `git describe`). |
| `deps` | Normal-recipe runtime deps → installed into sysroot. |
| `builddeps` | Build-only normal deps, not packed (build-only compilers/headers). |
| `hostdeps` | Host recipes to build; installed into container `/usr/local` for this build. |
| `hostrundeps` | Like `hostdeps` but also recorded as runtime deps of the produced host package. |
| `imagedeps` | Debian apt packages installed into the build container (`build-essential`, `ninja`, …). |
| `allow_network` | `yes`/`no` (default `no`) — container network access. |
| `cross_compile` | `yes`/`no` (default `no`) — force cross-compile flow even in native mode. |
| `bootstrap_pkg` | `yes` → not recorded as a dep / not installed into sysroot; bootstrap-only. |
| `clean_workdirs` | Per-recipe override to keep/discard workdir vs `JINX_CLEAN_WORKDIRS`. |
| `source_*` | `source_deps`/`source_imagedeps`/`source_hostdeps`/`source_allow_network` — replace deps/… only during source-preparation stages. |

## Recipe functions

Run in order: `early_prepare()` → (patches) → `prepare()` → `configure()` → `build()` → `package()`.

- `early_prepare()` — on pristine source, before patches (e.g. fetching vendored submodules).
- `prepare()` — after patches, from source dir (e.g. `meson subprojects download`, `autoreconf`).
- `configure()` — from build dir; only when build dir doesn't exist (first build, after `rebuild`,
  after version/revision bump). Do `./configure`, `meson setup`, `cmake -G Ninja …`.
- `build()` — every build: `make -j${parallelism}`, `meson compile`, `cmake --build …`.
- `package()` — install into `${dest_dir}`; Jinx turns it into the XBPS file.

No function is required — a recipe can exist purely to provide sources via `from_source`.

## Provided variables (read-only, available to recipe functions)

| Variable | Meaning |
|---|---|
| `name` | Package name (dir basename). |
| `recipe_dir` | Host-side absolute path of recipe dir; **not accessible from inside the build container** — reference static package files via `${base_dir}/recipes/<name>/<file>` instead (see `add-package.md`). |
| `source_dir` | Unpacked source tree path. |
| `prefix` | `/usr` (normal) or `/usr/local` (host). |
| `sysroot` | Populated sysroot in container (`/sysroot`). |
| `dest_dir` | Where `package()` installs to. |
| `parallelism` | Suggested `make -j` value; from `JINX_PARALLELISM` or auto. |
| `base_dir` / `build_dir` | Source dir / build dir paths. |
| `JINX_ARCH` | Target arch. |
