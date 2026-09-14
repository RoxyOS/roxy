use crate::{SyscallResult, args::Out, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Openpty, handle(output: Out<[i32; 2]> => Fault));

/// Allocates a pseudo-terminal pair and writes its master and slave descriptors into `output`.
///
/// `output[0]` is the master (a bidirectional byte stream with no line discipline) and `output[1]`
/// is the slave (the terminal whose line discipline, termios, and controlling-session state the
/// program runs under). The pair has no device-filesystem name, so callers that need the slave in
/// a child pass this descriptor across `fork` instead of reopening a path; the child makes it its
/// controlling terminal with `TIOCSCTTY`.
fn handle(output: Out<[i32; 2]>) -> SyscallResult {
    output.validate()?;

    let (master, slave) = roxy_pty::open_pair();
    let master_fd = roxy_process::insert_open_file(master, false);
    let slave_fd = roxy_process::insert_open_file(slave, false);

    let descriptors = if let (Ok(master), Ok(slave)) = (
        i32::try_from(master_fd.as_u32()),
        i32::try_from(slave_fd.as_u32()),
    ) {
        [master, slave]
    } else {
        close_pair(master_fd, slave_fd);

        return Err(Errno::Overflow);
    };

    // SAFETY: The array contains two initialized i32 values and has no padding.
    if let Err(error) = unsafe { output.write(&descriptors) } {
        close_pair(master_fd, slave_fd);

        return Err(error);
    }

    Ok(0)
}

fn close_pair(master: roxy_fd::Fd, slave: roxy_fd::Fd) {
    roxy_process::close_file(master).expect("new pty master descriptor must remain open");
    roxy_process::close_file(slave).expect("new pty slave descriptor must remain open");
}
