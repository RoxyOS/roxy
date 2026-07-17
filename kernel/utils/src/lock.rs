use core::ops::{Deref, DerefMut};

use spin::{Mutex, MutexGuard};

use crate::preemption::{self, PreemptionGuard};

pub struct Lock<T: ?Sized> {
    inner: Mutex<T>,
}

#[must_use]
pub struct LockGuard<'a, T: ?Sized> {
    // Fields drop in declaration order, so the lock is released before preemption is restored.
    guard: MutexGuard<'a, T>,
    _preemption: PreemptionGuard,
}

impl<T> Lock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(value),
        }
    }
}

impl<T: ?Sized> Lock<T> {
    pub fn lock(&self) -> LockGuard<'_, T> {
        let preemption = preemption::disable();
        let guard = self.inner.lock();
        LockGuard {
            guard,
            _preemption: preemption,
        }
    }

    pub fn try_lock(&self) -> Option<LockGuard<'_, T>> {
        let preemption = preemption::disable();
        let guard = self.inner.try_lock()?;
        Some(LockGuard {
            guard,
            _preemption: preemption,
        })
    }
}

impl<T: ?Sized> Deref for LockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<T: ?Sized> DerefMut for LockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}
