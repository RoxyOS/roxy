use roxy_fd::{Fd, FdTable};

pub(crate) fn inject(table: &mut FdTable) {
    let terminal = roxy_fbterm::terminal().unwrap_or_else(roxy_serial::terminal);

    for expected in [Fd::new(0), Fd::new(1), Fd::new(2)] {
        let inserted = table.insert(roxy_terminal::open(terminal.clone()));

        assert_eq!(inserted, expected, "initial FD table was not empty");
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::sync::Arc;

    use roxy_fd::{Fd, FdTable};

    use super::inject;

    roxy_test::kernel_test!("roxy-kernel::initial-standard-fds", initial_standard_fds, {
        let mut table = FdTable::new();

        inject(&mut table);

        let stdin = table.get(Fd::new(0)).unwrap();
        let stdout = table.get(Fd::new(1)).unwrap();
        let stderr = table.get(Fd::new(2)).unwrap();
        assert!(stdin.is_terminal());
        assert!(stdout.is_terminal());
        assert!(stderr.is_terminal());
        assert!(!Arc::ptr_eq(&stdin, &stdout));
        assert!(!Arc::ptr_eq(&stdout, &stderr));
        assert!(table.get(Fd::new(3)).is_none());
    });
}
