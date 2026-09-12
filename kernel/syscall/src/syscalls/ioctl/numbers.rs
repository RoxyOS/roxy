//! Request numbers of the Roxy ioctl space.
//!
//! The space is flat and global: one number means exactly one operation for the whole kernel,
//! whichever descriptor it arrives on, because dispatch matches the raw number before any file
//! object sees the request. Each device family therefore owns a `0x100`-aligned block and defines
//! at most `0x100` requests; a family that outgrows its block takes a new one instead of extending
//! into its neighbour. [`BLOCK_SIZE`] and the family bases are what make the space disjoint by
//! construction, and the test below checks that no request has spilled out of its block. A block's
//! base is the family's first request, so the first request of a family is spelled as the base
//! itself rather than as `base + 0`.
//!
//! The whole space is also one narrow window, `[SPACE_BASE, SPACE_END)`, placed above every number
//! another personality produces for the requests this kernel serves. Linux's plain `_IO(type, nr)`
//! form cannot exceed `0xFFFF`, and its `_IOW`, `_IOR`, and `_IOWR` forms set bits 30 and 31 on top
//! of a 16-bit size field, so a request below the base is never one of ours; [`in_space`] is what
//! separates a foreign request from an undefined one of our own. The base stays below
//! `0x4000_0000` and the space below `2^31`, so a request still fits a signed `int` in ported code
//! that stores one.
//!
//! No direction or size bits are encoded, unlike the Linux `_IOC` scheme: this layer matches whole
//! numbers, and each handler already knows the direction and record layout of its own request, so
//! an encoding would only look self-describing while adding a second source of truth for the
//! layout it describes.
//!
//! Numbers are opaque below this boundary: the FD, TTY, and device layers dispatch on
//! `roxy_fd::IoctlRequest` and never see them. Userspace mirrors these numbers in the Roxy mlibc
//! `abi-bits/ioctls.h`, the other half of the same hand-maintained contract.

/// Base of the Roxy ioctl space: a request is this base plus its family's block and offset.
///
/// It sits above every plain Linux `_IO(type, nr)` request, whose 8-bit type and number fields cap
/// it at `0xFFFF`, so no such request can be mistaken for ours. Userspace spells it
/// `ROXY_IOCTL_BASE`.
pub(super) const SPACE_BASE: u64 = 0x10000;

/// Size of the block each device family owns; a family defines fewer than this many requests.
pub(super) const BLOCK_SIZE: u64 = 0x100;

/// Terminal requests: `TCGETS` through `TCFLSH`.
pub(super) const TERMINAL_BASE: u64 = SPACE_BASE;

/// Pseudo-terminal requests: `TIOCGPTN` and `TIOCSPTLCK`.
pub(super) const PTY_BASE: u64 = TERMINAL_BASE + BLOCK_SIZE;

/// Framebuffer requests: the `ROXY_FRAMEBUFFER_*` family.
pub(super) const FRAMEBUFFER_BASE: u64 = PTY_BASE + BLOCK_SIZE;

/// Requests that act on the open file description itself rather than on a device: `FIONBIO`.
pub(super) const DESCRIPTION_BASE: u64 = FRAMEBUFFER_BASE + BLOCK_SIZE;

/// One past the last request a family may define; the space is `[SPACE_BASE, SPACE_END)`.
pub(super) const SPACE_END: u64 = DESCRIPTION_BASE + BLOCK_SIZE;

/// The base is above every plain Linux request, so a request below it is never one of ours.
const _: () = assert!(SPACE_BASE > 0xFFFF);

/// The space ends below Linux's `_IOC` direction bit and below `2^31`, so no request of ours looks
/// like an encoded foreign request and every one of them fits a signed `int`.
const _: () = assert!(SPACE_END <= 0x4000_0000);

/// Whether `request` is one the Roxy space could have assigned.
///
/// A request outside the space was produced by another personality's numbering — a program
/// compiled against another libc, or one that hardcoded that libc's constants — and dispatch must
/// report it as foreign rather than as an undefined request of our own.
pub(super) fn in_space(request: u64) -> bool {
    (SPACE_BASE..SPACE_END).contains(&request)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{
        BLOCK_SIZE, DESCRIPTION_BASE, FRAMEBUFFER_BASE, PTY_BASE, SPACE_BASE, SPACE_END,
        TERMINAL_BASE, in_space,
    };
    use crate::syscalls::ioctl::{execute, framebuffer, pty, terminal};

    /// Every request the kernel supports, grouped by the block that owns it. A new request must be
    /// added here, and the array below grown, for the check to cover it.
    const BLOCKS: [(u64, &[u64]); 4] = [
        (
            TERMINAL_BASE,
            &[
                terminal::TCGETS,
                terminal::TCSETS,
                terminal::TCSETSW,
                terminal::TCSETSF,
                terminal::TIOCGWINSZ,
                terminal::TIOCSWINSZ,
                terminal::TIOCGPGRP,
                terminal::TIOCSPGRP,
                terminal::TIOCSCTTY,
                terminal::TCFLSH,
            ],
        ),
        (PTY_BASE, &[pty::TIOCGPTN, pty::TIOCSPTLCK]),
        (
            FRAMEBUFFER_BASE,
            &[
                framebuffer::ROXY_FRAMEBUFFER_GET_INFO,
                framebuffer::ROXY_FRAMEBUFFER_TAKE_CONTROL,
                framebuffer::ROXY_FRAMEBUFFER_RELEASE_CONTROL,
            ],
        ),
        (DESCRIPTION_BASE, &[execute::FIONBIO]),
    ];

    kernel_test!(
        "roxy-syscall::ioctl-request-space",
        requests_stay_in_their_block,
        {
            let mut seen = [0u64; 16];
            let mut count = 0;

            for (base, requests) in BLOCKS {
                for request in requests {
                    assert!(
                        *request >= base && *request - base < BLOCK_SIZE,
                        "ioctl request outside the block that owns it"
                    );
                    assert!(in_space(*request), "ioctl request outside the Roxy space");
                    assert!(
                        !seen[..count].contains(request),
                        "duplicate ioctl request number"
                    );
                    seen[count] = *request;
                    count += 1;
                }
            }

            assert_eq!(
                count,
                seen.len(),
                "ioctl request table is not fully populated"
            );
        }
    );

    kernel_test!(
        "roxy-syscall::ioctl-request-space",
        separates_foreign_requests,
        {
            assert!(in_space(SPACE_BASE));
            assert!(in_space(SPACE_END - 1));
            assert!(!in_space(SPACE_BASE - 1));
            assert!(!in_space(SPACE_END));

            // Plain Linux requests. `0x0301` is Linux's `HDIO_GETGEO`, which the previous base
            // placed inside the framebuffer block; `0x5401` is `TCGETS`.
            assert!(!in_space(0x0301));
            assert!(!in_space(0x5401));

            // An encoded Linux request: `TIOCGPTN`, with a direction bit and a size field.
            assert!(!in_space(0x8004_5430));
        }
    );
}
