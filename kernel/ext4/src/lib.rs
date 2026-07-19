#![no_std]
#![allow(clippy::missing_errors_doc)]

extern crate alloc;

mod adapter;
mod error;
mod file;
mod metadata;
mod operations;
mod operations_inner;
#[cfg(feature = "kernel-test")]
mod tests;
mod utils;

use alloc::{boxed::Box, sync::Arc};

use ext4plus::{Ext4, Ext4Read, Ext4Write};
use roxy_block::BlockDevice;
use roxy_utils::Lock;
use roxy_vfs::VfsError;

use adapter::DeviceIo;
use error::map_ext4;

pub struct Ext4FileSystem {
    filesystem: Ext4,
    device: &'static dyn BlockDevice,
    mutation: Arc<Lock<()>>,
}

impl Ext4FileSystem {
    pub fn load(device: &'static dyn BlockDevice) -> Result<Self, VfsError> {
        let io = Arc::new(DeviceIo::new(device));
        let reader: Box<dyn Ext4Read> = Box::new(io.clone());
        let writer: Box<dyn Ext4Write> = Box::new(io);
        let filesystem = Ext4::load_with_writer(reader, Some(writer)).map_err(map_ext4)?;

        Ok(Self {
            filesystem,
            device,
            mutation: Arc::new(Lock::new(())),
        })
    }
}
