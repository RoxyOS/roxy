use roxy_process::current_process_id;
use roxy_thread::scheduler::current_thread_id;

use crate::errno::Errno;

pub(crate) fn unsupported_argument(operation: &str, argument: u64, errno: Errno) -> Errno {
    roxy_utils::unsupported::report(
        operation,
        argument,
        current_process_id(),
        current_thread_id(),
        errno.number(),
    );
    errno
}
