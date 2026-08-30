use roxy_fd::Fd;
use roxy_pipe::pair as pipe_pair;

use crate::{
    SyscallResult,
    args::{Out, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::Pipe, handle(output: PipesOut => Fault, flags: usize => Invalid));

/// The two descriptors produced by `pipe`: the read end and the write end.
///
/// Wraps the raw `Out<[i32; 2]>` slot so the handler can write the pair without reasoning
/// about fd-number conversion and cleanup inline.
#[derive(Clone, Copy)]
struct PipesOut(Out<[i32; 2]>);

impl PipesOut {
    fn validate(self) -> Result<(), Errno> {
        self.0.validate()
    }

    /// Writes the read and write descriptors into user memory.
    ///
    /// On any failure the already-inserted descriptors are closed so they do not leak.
    fn write_pipes(self, read_fd: Fd, write_fd: Fd) -> Result<(), Errno> {
        let descriptors = match (
            i32::try_from(read_fd.as_u32()),
            i32::try_from(write_fd.as_u32()),
        ) {
            (Ok(read), Ok(write)) => [read, write],
            _ => {
                close_pair(read_fd, write_fd);
                return Err(Errno::Overflow);
            }
        };

        // SAFETY: the array is fully initialized and has no padding.
        if let Err(error) = unsafe { self.0.write(&descriptors) } {
            close_pair(read_fd, write_fd);
            return Err(error);
        }
        Ok(())
    }
}

impl SyscallArg for PipesOut {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        <Out<[i32; 2]> as SyscallArg>::parse(raw, error).map(Self)
    }
}

fn handle(output: PipesOut, flags: usize) -> SyscallResult {
    if flags != 0 {
        return Err(Errno::Invalid);
    }
    output.validate()?;

    let (read_end, write_end) = pipe_pair();
    let read_fd = roxy_process::insert_open_file(read_end, false);
    let write_fd = roxy_process::insert_open_file(write_end, false);

    output.write_pipes(read_fd, write_fd)?;
    Ok(0)
}

fn close_pair(read_fd: Fd, write_fd: Fd) {
    roxy_process::close_file(read_fd).expect("new pipe descriptor must remain open");
    roxy_process::close_file(write_fd).expect("new pipe descriptor must remain open");
}
