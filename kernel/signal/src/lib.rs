#![no_std]

mod action;

pub use action::SignalAction;

/// A process-directed signal supported by the current kernel.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signal {
    Interrupt = 2,
    Kill = 9,
    Terminate = 15,
}

impl Signal {
    #[must_use]
    pub const fn default_action(self) -> SignalAction {
        match self {
            Self::Interrupt | Self::Kill | Self::Terminate => SignalAction::Terminate,
        }
    }

    #[must_use]
    pub const fn number(self) -> u8 {
        self as u8
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{Signal, SignalAction};

    kernel_test!("roxy-signal::default-actions", default_actions, {
        assert_eq!(Signal::Interrupt.default_action(), SignalAction::Terminate);
        assert_eq!(Signal::Kill.default_action(), SignalAction::Terminate);
        assert_eq!(Signal::Terminate.default_action(), SignalAction::Terminate);
    });
}
