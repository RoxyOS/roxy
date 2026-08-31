# Known Issues

## ext4plus does not reclaim removed directories

`ext4plus 0.1.0-rc.2` can remove a directory entry without reclaiming the directory inode and its
allocated blocks. Repeated `rmdir` operations can therefore consume space until the volatile root
RAM disk is rebuilt at the next boot.

The adapter intentionally applies no reclamation workaround. The affected call site is marked with
a `FIXME` in `kernel/ext4/src/mutations.rs`.

## ext4plus final unlink of inline symlinks is unsafe

Short symbolic-link targets are stored inline in the inode. When their final directory entry is
unlinked, `ext4plus 0.1.0-rc.2` can interpret the inline target bytes as block pointers. This can
trigger an out-of-bounds block-group assertion or corrupt block accounting.

The adapter intentionally forwards the unlink without detection or a link-count workaround. The
affected call site is marked with a `FIXME` in `kernel/ext4/src/mutations.rs`.

## Foreground process groups exist but session validation is incomplete

Process groups, `setpgid`/`getpgid`/`setsid`, and TTY foreground-group selection
(`TIOCSPGRP`/`TIOCGPGRP`) are implemented, and Ctrl+C is delivered to the foreground group. When
no foreground group has been selected, the TTY falls back to the current reader. The session
model remains minimal: `setsid` skips the POSIX "caller must not already be a process group
leader" check because the spawn model makes every top-level process a leader, and `setpgid` does
not validate that target and group share a session. Both gaps are marked with `TODO(session)` in
`kernel/process/src/setpgid.rs`.
