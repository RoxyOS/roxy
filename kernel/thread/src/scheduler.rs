use alloc::vec::Vec;
use core::ptr;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_utils::{Lock, preemption};

use crate::{SavedContext, Thread, ThreadCreateError};

static SCHEDULER: Lock<Scheduler> = Lock::new(Scheduler::new());

struct Scheduler {
    threads: Vec<Thread>,
    current: Option<ThreadIndex>,
    control_context: Option<SavedContext>,
    pending_reap: Option<ThreadIndex>,
}

/// Index of the a thread in `Scheduler::threads`.
#[derive(Clone, Copy, Eq, PartialEq)]
struct ThreadIndex(usize);

impl Scheduler {
    const fn new() -> Self {
        Self {
            threads: Vec::new(),
            current: None,
            control_context: None,
            pending_reap: None,
        }
    }

    /// Prepares the first switch from the scheduler context to thread zero.
    ///
    /// This context stays suspended until the final runnable thread exits.
    fn initial_contexts(&mut self) -> (*mut SavedContext, *const SavedContext) {
        assert!(self.current.is_none(), "scheduler started twice");
        assert!(!self.threads.is_empty(), "scheduler has no threads");
        assert!(self.pending_reap.is_none(), "exited thread was not reaped");

        self.control_context = Some(SavedContext::empty());
        self.current = Some(ThreadIndex(0));

        let previous = ptr::from_mut(self.control_context.as_mut().unwrap());
        let next = ptr::from_mut(self.current_thread().unwrap().context());

        (previous, next)
    }

    /// Prepares a round-robin switch from the current thread to its successor.
    ///
    /// An exited predecessor is reaped first because the CPU is now running on the current
    /// thread's kernel stack. `None` means no distinct runnable successor exists.
    fn next_contexts(&mut self) -> Option<(*mut SavedContext, *const SavedContext)> {
        self.reap_pending();

        let current = self.current?;
        if self.threads.len() < 2 {
            return None;
        }

        let next = ThreadIndex((current.0 + 1) % self.threads.len());
        let previous_context = ptr::from_mut(self.current_thread()?.context());
        let next_context = ptr::from_mut(self.thread_from_index(next).context());
        self.current = Some(next);
        Some((previous_context, next_context))
    }

    /// Prepares a switch away from an exiting thread without dropping its active kernel stack.
    ///
    /// A remaining runnable thread becomes the successor. Only the final exiting thread switches
    /// back to the suspended scheduler context. The exited thread is reaped from a different stack
    /// at the next scheduler entry.
    fn exit_contexts(&mut self) -> (*mut SavedContext, *const SavedContext) {
        self.reap_pending();

        let current = self.current.expect("no current thread");
        let next =
            (self.threads.len() > 1).then(|| ThreadIndex((current.0 + 1) % self.threads.len()));
        self.current = next;
        self.pending_reap = Some(current);

        let previous = ptr::from_mut(self.thread_from_index(current).context());
        let next = match next {
            Some(next) => ptr::from_mut(self.thread_from_index(next).context()).cast_const(),
            None => ptr::from_ref(
                self.control_context
                    .as_ref()
                    .expect("scheduler not started"),
            ),
        };

        (previous, next)
    }

    fn finish_scheduling(&mut self) {
        self.reap_pending();
        assert!(self.threads.is_empty(), "scheduler context resumed early");
        self.control_context = None;
    }

    /// Reaps a thread only after execution has moved away from its kernel stack.
    ///
    /// `exit_current` cannot remove the active thread immediately because `rsp` still points into
    /// that thread's owned kernel stack. It records the thread in `pending_reap` and switches to a
    /// successor or the scheduler context. The next scheduler entry runs on that different stack,
    /// removes the exited thread, and adjusts `current` if `Vec::remove` shifted its index.
    fn reap_pending(&mut self) {
        let Some(pending_reap) = self.pending_reap.take() else {
            return;
        };

        assert!(
            self.current != Some(pending_reap),
            "cannot reap the active thread"
        );
        self.threads.remove(pending_reap.0);
        if let Some(current) = &mut self.current
            && current.0 > pending_reap.0
        {
            current.0 -= 1;
        }
    }

    fn thread_from_index(&mut self, index: ThreadIndex) -> &mut Thread {
        &mut self.threads[index.0]
    }

    fn current_thread(&mut self) -> Option<&mut Thread> {
        let index = self.current?;
        Some(self.thread_from_index(index))
    }
}

/// Adds a permanently runnable thread to the round-robin queue.
///
/// # Errors
///
/// Returns an error when the thread's kernel stack cannot be allocated.
pub fn spawn(entry: fn() -> !) -> Result<(), ThreadCreateError> {
    CurrentArchitectureBackend::without_interrupts(|| {
        let thread = Thread::new(entry)?;
        SCHEDULER.lock().threads.push(thread);
        Ok(())
    })
}

/// Adds an already constructed thread to the runnable queue.
pub fn enqueue(thread: Thread) {
    CurrentArchitectureBackend::without_interrupts(|| SCHEDULER.lock().threads.push(thread));
}

/// Starts the permanent scheduler and never returns.
///
/// # Panics
///
/// Panics if the scheduler was already started or has no runnable threads.
pub fn start() -> ! {
    CurrentArchitectureBackend::without_interrupts(|| {
        let (previous, next) = SCHEDULER.lock().initial_contexts();
        // SAFETY: The scheduler owns both live contexts and holds no lock across the switch.
        unsafe { SavedContext::switch(previous, next) };
        SCHEDULER.lock().finish_scheduling();
        CurrentArchitectureBackend::halt_forever()
    })
}

/// Terminates the current thread and switches to its successor or the suspended `start` caller.
///
/// # Panics
///
/// Panics when called without an active thread or with interrupts enabled.
pub fn exit_current() -> ! {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    let (previous, next) = SCHEDULER.lock().exit_contexts();
    // SAFETY: both contexts stay scheduler-owned until execution resumes on a different stack.
    unsafe { SavedContext::switch(previous, next) };
    panic!("exited thread resumed")
}

/// Switches to the next runnable thread after a timer interrupt.
///
/// # Panics
///
/// Panics if called with interrupts enabled.
pub fn on_timer_interrupt() {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    if preemption::is_disabled() {
        return;
    }

    let Some((previous, next)) = SCHEDULER.lock().next_contexts() else {
        return;
    };

    // SAFETY: The scheduler owns both live contexts and holds no lock across the switch.
    unsafe { SavedContext::switch(previous, next) };
}
