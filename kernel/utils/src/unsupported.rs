use core::{fmt, fmt::Debug};

use spin::Once;

type Reporter = for<'a> fn(fmt::Arguments<'a>);

static REPORTER: Once<Reporter> = Once::new();

pub fn initialize(reporter: Reporter) {
    REPORTER.call_once(|| reporter);
}

/// Emits a mandatory diagnostic for an unsupported userspace operation.
///
/// # Panics
///
/// Panics when the kernel has not registered its serial reporter.
pub fn report<Argument, ProcessId, ThreadId>(
    operation: &str,
    argument: Argument,
    process_id: ProcessId,
    thread_id: ThreadId,
    errno: u64,
) where
    Argument: fmt::Display,
    ProcessId: Debug,
    ThreadId: Debug,
{
    let reporter = REPORTER
        .get()
        .copied()
        .expect("unsupported-operation reporter must be initialized");
    reporter(format_args!(
        "UNSUPPORTED operation={operation} argument={argument} pid={process_id:?} tid={thread_id:?} errno={errno}\n"
    ));
}
