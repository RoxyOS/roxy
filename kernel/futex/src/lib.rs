#![no_std]

extern crate alloc;

mod table;

use roxy_memory::UserAddress;
use roxy_thread::{ThreadId, scheduler};
use roxy_utils::Lock;
use roxy_vm::{AddrSpaceHandle, VmError};

use table::{FutexKey, FutexTable};

static FUTEXES: Lock<FutexTable> = Lock::new(FutexTable::new());

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FutexError {
    Fault,
    Invalid,
    Mismatch,
}

/// Installs futex cleanup into the thread exit lifecycle.
pub fn initialize() {
    scheduler::register_exit_handler(cancel_thread);
}

/// Waits while an aligned user word equals `expected`.
///
/// # Errors
///
/// Returns [`FutexError::Fault`] when the word is not mapped, [`FutexError::Invalid`] when the
/// address is unaligned, or [`FutexError::Mismatch`] when the current word differs from expected.
pub fn wait(
    addrspace: &AddrSpaceHandle,
    address: UserAddress,
    expected: u32,
) -> Result<(), FutexError> {
    let key = FutexKey::new(addrspace.id(), address)?;
    let mut futexes = FUTEXES.lock();
    let mut value = [0; 4];
    addrspace
        .read_bytes(address, &mut value)
        .map_err(map_vm_error)?;

    if u32::from_ne_bytes(value) != expected {
        return Err(FutexError::Mismatch);
    }

    let thread_id = scheduler::current_thread_id();
    futexes.enqueue(key, thread_id);
    let pending = scheduler::prepare_block_current();
    drop(futexes);
    pending.perform();

    Ok(())
}

/// Wakes at most `count` threads waiting on one address-space-local key.
///
/// # Errors
///
/// Returns [`FutexError::Invalid`] when the address is not aligned.
pub fn wake(
    addrspace: &AddrSpaceHandle,
    address: UserAddress,
    count: usize,
) -> Result<usize, FutexError> {
    let key = FutexKey::new(addrspace.id(), address)?;
    let mut futexes = FUTEXES.lock();
    let mut woken = 0;

    while woken < count {
        let Some(thread_id) = futexes.dequeue(key) else {
            break;
        };

        if scheduler::wake(thread_id) {
            woken += 1;
        }
    }

    Ok(woken)
}

fn cancel_thread(thread_id: ThreadId) {
    FUTEXES.lock().remove(thread_id);
}

fn map_vm_error(_: VmError) -> FutexError {
    FutexError::Fault
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::UserAddress;
    use roxy_test::kernel_test;
    use roxy_thread::Thread;
    use roxy_vm::AddrSpace;

    use super::{FutexKey, FutexTable};

    kernel_test!("roxy-futex::queue-order", queue_order, {
        let addrspace = AddrSpace::new().unwrap().into_handle();
        let key = FutexKey::new(addrspace.id(), UserAddress::new(0x40_0000).unwrap()).unwrap();
        let first = Thread::new(unused_thread).unwrap();
        let second = Thread::new(unused_thread).unwrap();
        let first_id = first.id();
        let second_id = second.id();
        let mut table = FutexTable::new();

        table.enqueue(key, first_id);
        table.enqueue(key, second_id);
        assert_eq!(table.dequeue(key), Some(first_id));
        table.remove(second_id);
        assert_eq!(table.dequeue(key), None);
    });

    fn unused_thread() -> ! {
        panic!("unused futex test thread started")
    }
}
