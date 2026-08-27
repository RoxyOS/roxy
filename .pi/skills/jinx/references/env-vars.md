# Jinx Environment variables & Jinxfile

## Environment variables

| Variable | Meaning |
|---|---|
| `JINX_PARALLELISM` | Override `parallelism` (suggested `make -j` / `meson compile -j` value). |
| `JINX_CACHE_DIR` | Cache location (default `.jinx-cache`). |
| `JINX_NATIVE_MODE=yes` | Mount sysroot directly as build root for non-cross recipes. |
| `JINX_CLEAN_WORKDIRS=yes` | Remove build dir + downloaded sources after a successful build (per-recipe opt-out via `clean_workdirs=no`). |
| `JINX_ARCH` | Target arch, from `init` (`JINX_ARCH` in `.jinx-parameters`, not the Jinxfile). |

## Jinxfile (distro/Jinxfile)

Project-level config read by Jinx. Roxy OS pins:

- `JINX_MAJOR_VER` — Jinx major version (currently `0.10`).
- `JINX_DEBIAN_SNAPSHOT` — pinned Debian snapshot date (currently `20251101T000000Z`); the repo's
  `flake.nix` patches the snapshot mirror to `mirrors.aliyun.com/debian` and uses `--foreign
  bookworm`.
