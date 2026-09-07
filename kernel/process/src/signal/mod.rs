//! Signal handling: queueing, disposition, per-thread masks, and frame delivery.
//!
//! `roxy-process` keeps the signal model process- and thread-scoped: process-directed signals sit
//! in a process queue, thread-directed signals in per-thread pending queues, and each thread has
//! its own signal mask. This module owns those records and the rules for queueing, delivering at
//! a userspace-return boundary, and blocking until a pending signal can be consumed.

mod action;
mod deliver;
mod mask;
mod send;
mod wait;

pub use action::{SignalAction, replace_signal_action, signal_action_of};
pub use deliver::{deliver_pending_signal, pop_signal_frame};
pub use mask::{
    block_signals, currently_blocked_signals, replace_masked_signals,
    thread_belongs_to_current_process, unblock_signals,
};
pub use send::{
    SignalError, send_signal, send_signal_to_pgid, send_thread_signal, send_thread_timer_signal,
    send_timer_signal,
};
pub use wait::{
    SigWait, has_pending_signal, has_unmasked_pending_signal, take_matching_pending_signal,
};

// Internal records shared across the submodules (and by `signal_frame`).
pub(crate) use send::{PendingSignal, SignalSource};
