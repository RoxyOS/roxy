#![no_std]

extern crate alloc;

use alloc::{boxed::Box, collections::VecDeque, sync::Arc};

use roxy_poll::{PollListener, PollListeners, PollRegistration};
use roxy_thread::scheduler;
use roxy_utils::Lock;

use roxy_fd::{
    File, FileError, FileMetadata, FileType, OpenFile, PollEvents, SeekError, SeekFrom, StatusFlags,
};

const CAPACITY: usize = 64 * 1024;

struct PipeState {
    bytes: VecDeque<u8>,
    readers: usize,
    writers: usize,
    read_listeners: Arc<PollListeners>,
    write_listeners: Arc<PollListeners>,
}

struct PipeEnd {
    state: Arc<Lock<PipeState>>,
    role: PipeRole,
}

#[derive(Clone, Copy)]
enum PipeRole {
    Read,
    Write,
}

#[must_use]
pub fn pair() -> (Arc<OpenFile>, Arc<OpenFile>) {
    let state = Arc::new(Lock::new(PipeState {
        bytes: VecDeque::new(),
        readers: 1,
        writers: 1,
        read_listeners: Arc::new(PollListeners::new()),
        write_listeners: Arc::new(PollListeners::new()),
    }));

    (
        OpenFile::new(Box::new(PipeEnd {
            state: state.clone(),
            role: PipeRole::Read,
        })),
        {
            let end = OpenFile::new(Box::new(PipeEnd {
                state,
                role: PipeRole::Write,
            }));
            end.set_status_flags(StatusFlags::WRITE_ONLY);
            end
        },
    )
}

impl PipeEnd {
    fn wait(&self) -> (scheduler::PendingBlock, PollRegistration) {
        let listener = PollListener::current_thread();
        let listeners = {
            let state = self.state.lock();
            match self.role {
                PipeRole::Read => state.read_listeners.clone(),
                PipeRole::Write => state.write_listeners.clone(),
            }
        };

        let registration = listeners.register(listener.clone());
        (
            scheduler::prepare_block_current_with_key(listener.wait_key()),
            registration,
        )
    }
}

impl File for PipeEnd {
    fn poll(&mut self) -> Result<PollEvents, FileError> {
        let state = self.state.lock();

        Ok(match self.role {
            PipeRole::Read => PollEvents {
                readable: !state.bytes.is_empty() || state.writers == 0,
                hangup: state.writers == 0,
                ..PollEvents::default()
            },

            PipeRole::Write => PollEvents {
                writable: state.readers != 0 && state.bytes.len() < CAPACITY,
                hangup: state.readers == 0,
                ..PollEvents::default()
            },
        })
    }

    fn register_poll_listener(&mut self, listener: Arc<PollListener>) -> PollRegistration {
        let state = self.state.lock();

        match self.role {
            PipeRole::Read => state.read_listeners.register(listener),
            PipeRole::Write => state.write_listeners.register(listener),
        }
    }

    fn is_terminal(&self) -> bool {
        false
    }

    fn metadata(&self) -> Result<FileMetadata, FileError> {
        Ok(FileMetadata {
            file_id: 0,
            file_type: FileType::Fifo,
            permissions: 0,
            size: 0,
            hard_links: 1,
        })
    }

    fn read(&mut self, _position: &mut u64, output: &mut [u8]) -> Result<usize, FileError> {
        if !matches!(self.role, PipeRole::Read) {
            return Err(FileError::BadOperation);
        }

        if output.is_empty() {
            return Ok(0);
        }

        loop {
            {
                let mut state = self.state.lock();

                if !state.bytes.is_empty() {
                    let count = output.len().min(state.bytes.len());

                    for byte in &mut output[..count] {
                        *byte = state.bytes.pop_front().unwrap();
                    }

                    let listeners = state.write_listeners.clone();

                    drop(state);
                    listeners.notify();

                    return Ok(count);
                }

                if state.writers == 0 {
                    return Ok(0);
                }
            }

            let (pending, registration) = self.wait();
            pending.perform();
            drop(registration);
        }
    }

    fn write(&mut self, _position: &mut u64, input: &[u8]) -> Result<usize, FileError> {
        if !matches!(self.role, PipeRole::Write) {
            return Err(FileError::BadOperation);
        }

        if input.is_empty() {
            return Ok(0);
        }

        loop {
            let mut state = self.state.lock();

            if state.readers == 0 {
                return Err(FileError::BrokenPipe);
            }

            if state.bytes.len() < CAPACITY {
                let count = input.len().min(CAPACITY - state.bytes.len());
                state.bytes.extend(&input[..count]);
                let listeners = state.read_listeners.clone();

                drop(state);
                listeners.notify();

                return Ok(count);
            }

            drop(state);

            let (pending, registration) = self.wait();
            pending.perform();
            drop(registration);
        }
    }

    fn seek(&mut self, _current: u64, _position: SeekFrom) -> Result<u64, SeekError> {
        Err(SeekError::NotSeekable)
    }
}

impl Drop for PipeEnd {
    fn drop(&mut self) {
        let (readers, writers) = {
            let mut state = self.state.lock();
            match self.role {
                PipeRole::Read => state.readers -= 1,
                PipeRole::Write => state.writers -= 1,
            }
            (state.read_listeners.clone(), state.write_listeners.clone())
        };
        readers.notify();
        writers.notify();
    }
}
