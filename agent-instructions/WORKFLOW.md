# Development Workflows

This document contains the repeatable procedures for changes that cross repository boundaries.
`AGENTS.md` defines the repository rules; this document explains how to apply those rules to
common development tasks. Read the applicable subsystem `DESIGN.md` before changing its design.

The step-by-step procedures live in the skills:

- **mlibc development** (local iteration, publishing, recipe pin updates, ABI validation): see
  `.pi/skills/mlibc/SKILL.md`.
- **Patching a distro package** (workdir edits, `jinx regen`, patch lifecycle, rootfs refresh):
  see `.pi/skills/jinx/SKILL.md` and `.pi/skills/jinx/patch.md`.

## mlibc fork policy and integration boundary

Roxy maintains mlibc as a thin fork. Keep Roxy-specific work as a reviewable commit stack on top
of the selected upstream base, and upstream generally useful changes when practical. Local
iteration, publication to the canonical RoxyOS fork, and recipe integration are separate phases.

A task that changes Roxy's mlibc sysdeps authorizes the ordinary Git mutations required by this
workflow: after validation, commit and push the mlibc change, verify the published commit, update
the recipe pin, and continue integration without requesting additional user confirmation. This
exception does not authorize history rewriting, force-pushes, unrelated repository changes, or
recovery from a moved remote branch.

After the clean build succeeds, leave the recipe update ready for the main Roxy OS repository's
normal integration boundary. If the user's task also explicitly requests committing the main
repository, commit and push it without another confirmation; otherwise do not infer permission to
commit unrelated or accompanying kernel changes. Keep the compatible kernel ABI change and recipe
pin in the same integration series. If integration exposes a defect, publish a follow-up mlibc
commit instead of rewriting a commit already referenced by a recipe.
