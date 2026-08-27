# Debug a build failure

Use when a package fails to build.

## Tips

- Reproduce interactively: `jinx run-in <recipe> bash` — a container with the recipe's
  `hostdeps`/`hostrundeps`/`imagedeps`, from the build dir.
- The recipe is bash run inside a throwaway Debian container — errors appear in the container
  output, not on the host.
- Error names a missing tool → add `imagedeps` (Debian apt) / `hostdeps` (host recipe) /
  `builddeps`.
