# Multi-Agent Collaboration

Use this protocol for every task. Always assume that other independently launched Codex instances
may be operating in the same worktree, even when no other agent is visible in the current instance.
Process-local agent discovery is not proof that the worktree is exclusive. The filesystem locks
defined here are the source of truth for edit ownership.

The purpose of the protocol is to make edit ownership visible across isolated processes, prevent
overlapping writes, and preserve changes made by other agents.

## Shared Worktree Rules

- Divide work along subsystem boundaries and give each active edit scope one owner.
- Inspect filesystem locks unconditionally. Never skip lock discovery because the current Codex
  instance believes that it is working alone.
- Treat all pre-existing and concurrent changes as belonging to their authors. Do not rewrite,
  stage, revert, move, or delete another agent's changes unless that agent explicitly hands them
  over.
- Keep searches and read-only investigation unrestricted, but acquire the applicable lock before
  the first filesystem write, including generated files, formatting, and automated fixes.
- Do not run repository-wide mutation commands while another agent owns any subsystem lock.
  Restrict formatters, generators, and fixers to the locked scope.
- Communicate changes to shared contracts, dependencies, manifests, generated interfaces, or
  repository-level files before making dependent agents adjust their work.

## Subsystem Scope

A subsystem is the smallest directory that owns the planned change. A crate, package, recipe,
architecture backend, or directory with its own `DESIGN.md` is normally a subsystem. If a change
spans several nested areas, lock their nearest common owning directory. Files at the repository
root require a repository-root lock.

A lock covers the directory containing it and that directory's entire tree. Lock scopes overlap
when they are identical or when either directory is an ancestor of the other. Overlapping locks
owned by different agents are forbidden.

## Agent Lock Protocol

At the start of a task, each Codex instance must generate a unique random UUID for its owner ID.
Prefer a UUIDv4. Reuse the same UUID for every lock owned by that instance during the task; a new
Codex window must generate a different UUID. Names such as `codex`, `root`, agent roles, timestamps,
or process IDs are not valid owner IDs.

Every agent **must** create a `.agent-lock` in the subsystem root before it begins editing that
subsystem. This requirement applies even when the agent has no evidence that another instance
exists. The lock must contain enough information to identify its owner and purpose:

```text
owner: <UUIDv4>
task: <short description of the active edit>
created_at: <ISO 8601 timestamp with timezone>
```

Do not include credentials or other secrets in a lock.

Acquire a lock as follows:

1. Scan the worktree for `.agent-lock` files, then inspect the target directory, every ancestor up
   to the repository root, and the target's descendants. Perform this scan unconditionally before
   every write phase. Any lock with an overlapping scope is a conflict.
2. Create the target `.agent-lock` with a create-if-absent operation. Never overwrite or truncate
   an existing lock. An `apply_patch` `Add File` operation is acceptable because it fails when the
   path already exists.
3. Confirm that the created lock's `owner` field contains the current instance's UUID before
   making any other write.
4. Keep the lock through implementation, formatting, generation, and validation.

When a task needs several independent subsystem locks, determine all required scopes first,
acquire them in lexicographic path order, and begin editing only after all acquisitions succeed.
If any acquisition fails, release the locks acquired for that task and coordinate the conflict.

## Conflicts And Stale Locks

- When a conflicting lock exists, do not edit the overlapping scope and do not create a nested
  lock to bypass it. Report the locked path and lock contents to the user so they can coordinate
  across independently launched Codex windows, or work on a non-overlapping task.
- Do not poll, wait indefinitely, or infer that a lock is stale from its age alone.
- Only the lock owner may normally remove it. Another instance may remove an abandoned lock only
  after the user explicitly confirms that its owner is no longer active and will not resume the
  edit.
- If an agent becomes unable to continue, it must report the locked path and the state of its
  changes so the user can arrange a handoff between instances.

## Release And Handoff

The owning agent **must** delete each `.agent-lock` as soon as it stops actively editing the
subsystem, whether the task succeeds, is cancelled, is handed off, or fails. Keep the lock until
the final write and relevant validation are complete, but remove it before the final handoff.

Locks are ephemeral coordination artifacts. Never stage or commit them. Before finishing, verify
that the owned locks are gone, report any intentionally retained worktree changes, and identify
the validation performed. Removing a lock releases edit ownership; it does not transfer ownership
of the changes left behind.
