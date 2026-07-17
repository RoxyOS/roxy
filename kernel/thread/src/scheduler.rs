use alloc::vec::Vec;
use core::ptr;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_utils::{Lock, preemption};

use crate::{SavedContext, Thread, ThreadCreateError};

static SCHEDULER: Lock<Scheduler> = Lock::new(Scheduler::new());

struct Scheduler {
    threads: Vec<Thread>,
    current: Option<ThreadIndex>,
    bootstrap: Option<SavedContext>,
}

/// Index of the a thread in `Scheduler::threads`.
#[derive(Clone, Copy)]
struct ThreadIndex(usize);

impl Scheduler {
    const fn new() -> Self {
        Self {
            threads: Vec::new(),
            current: None,
            bootstrap: None,
        }
    }

    fn initial_contexts(&mut self) -> (*mut SavedContext, *const SavedContext) {
        assert!(self.current.is_none(), "scheduler started twice");
        assert!(!self.threads.is_empty(), "scheduler has no threads");

        self.bootstrap = Some(SavedContext::empty());
        self.current = Some(ThreadIndex(0));

        let previous = ptr::from_mut(self.bootstrap.as_mut().unwrap());
        let next = ptr::from_mut(self.current_thread().unwrap().context());

        (previous, next)
    }

    fn next_contexts(&mut self) -> Option<(*mut SavedContext, *const SavedContext)> {
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

/// Starts the first runnable thread and never returns.
///
/// # Panics
///
/// Panics if the scheduler was already started or has no runnable threads.
pub fn start() -> ! {
    CurrentArchitectureBackend::without_interrupts(|| {
        let (previous, next) = SCHEDULER.lock().initial_contexts();
        // SAFETY: The scheduler owns both live contexts and holds no lock across the switch.
        unsafe { SavedContext::switch(previous, next) };
    });
    panic!("scheduler returned to its bootstrap context")
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
