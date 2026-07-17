#![no_std]

extern crate self as roxy_test;

pub struct TestCase {
    name: &'static str,
    run: fn(),
}

impl TestCase {
    #[must_use]
    pub const fn new(name: &'static str, run: fn()) -> Self {
        Self { name, run }
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub fn run(&self) {
        (self.run)();
    }
}

#[linkme::distributed_slice]
pub static TESTS: [TestCase];

#[macro_export]
macro_rules! kernel_test {
    ($name:literal, $function:ident, $body:block) => {
        fn $function() $body

        const _: () = {
            #[linkme::distributed_slice($crate::TESTS)]
            static TEST: $crate::TestCase = $crate::TestCase::new($name, $function);
        };
    };
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::sync::atomic::{AtomicBool, Ordering};

    use super::{TestCase, roxy_test};

    roxy_test::kernel_test!(
        "roxy-test::test-case-runs-function",
        test_case_runs_function,
        {
            static CALLED: AtomicBool = AtomicBool::new(false);

            fn mark_called() {
                CALLED.store(true, Ordering::Relaxed);
            }

            CALLED.store(false, Ordering::Relaxed);
            let test = TestCase::new("registered-name", mark_called);

            assert_eq!(test.name(), "registered-name");
            test.run();
            assert!(CALLED.load(Ordering::Relaxed));
        }
    );
}
