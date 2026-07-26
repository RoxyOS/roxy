#![no_std]

extern crate alloc;

mod listener;
mod queue;

pub use listener::PollListener;
pub use queue::{PollListeners, PollRegistration};
