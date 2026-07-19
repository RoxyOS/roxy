use alloc::boxed::Box;
use core::error::Error;

use ext4plus::{Ext4Read, Ext4Write};
use roxy_block::{BlockDevice, BlockError};
use roxy_utils::Lock;

const BLOCK_SIZE: usize = 512;
const BLOCK_SIZE_U64: u64 = 512;

pub(crate) struct DeviceIo {
    device: &'static dyn BlockDevice,
    io: Lock<()>,
}

impl DeviceIo {
    pub(crate) fn new(device: &'static dyn BlockDevice) -> Self {
        Self {
            device,
            io: Lock::new(()),
        }
    }

    fn validate(&self, start: u64, length: usize) -> Result<(), BlockError> {
        if self.device.block_size() != BLOCK_SIZE {
            return Err(BlockError::Unsupported);
        }

        let capacity = self
            .device
            .block_count()
            .checked_mul(BLOCK_SIZE_U64)
            .ok_or(BlockError::OutOfBounds)?;
        let length = u64::try_from(length).map_err(|_| BlockError::OutOfBounds)?;
        let end = start.checked_add(length).ok_or(BlockError::OutOfBounds)?;

        if end > capacity {
            return Err(BlockError::OutOfBounds);
        }

        Ok(())
    }

    fn read_range(&self, mut start: u64, mut destination: &mut [u8]) -> Result<(), BlockError> {
        if destination.is_empty() {
            return Ok(());
        }

        let offset = usize::try_from(start % BLOCK_SIZE_U64).map_err(|_| BlockError::Io)?;

        if offset != 0 {
            let mut block = [0_u8; BLOCK_SIZE];

            self.device
                .read_blocks(start / BLOCK_SIZE_U64, &mut block)?;

            let length = destination.len().min(BLOCK_SIZE - offset);

            destination[..length].copy_from_slice(&block[offset..offset + length]);
            start += u64::try_from(length).map_err(|_| BlockError::Io)?;
            destination = &mut destination[length..];
        }

        let aligned_length = destination.len() / BLOCK_SIZE * BLOCK_SIZE;

        if aligned_length != 0 {
            self.device
                .read_blocks(start / BLOCK_SIZE_U64, &mut destination[..aligned_length])?;

            start += u64::try_from(aligned_length).map_err(|_| BlockError::Io)?;
            destination = &mut destination[aligned_length..];
        }

        if !destination.is_empty() {
            let mut block = [0_u8; BLOCK_SIZE];

            self.device
                .read_blocks(start / BLOCK_SIZE_U64, &mut block)?;

            destination.copy_from_slice(&block[..destination.len()]);
        }

        Ok(())
    }

    fn write_range(&self, mut start: u64, mut source: &[u8]) -> Result<(), BlockError> {
        if source.is_empty() {
            return Ok(());
        }

        let offset = usize::try_from(start % BLOCK_SIZE_U64).map_err(|_| BlockError::Io)?;

        if offset != 0 {
            let mut block = [0_u8; BLOCK_SIZE];
            let index = start / BLOCK_SIZE_U64;

            self.device.read_blocks(index, &mut block)?;

            let length = source.len().min(BLOCK_SIZE - offset);

            block[offset..offset + length].copy_from_slice(&source[..length]);
            self.device.write_blocks(index, &block)?;

            start += u64::try_from(length).map_err(|_| BlockError::Io)?;
            source = &source[length..];
        }

        let aligned_length = source.len() / BLOCK_SIZE * BLOCK_SIZE;

        if aligned_length != 0 {
            self.device
                .write_blocks(start / BLOCK_SIZE_U64, &source[..aligned_length])?;

            start += u64::try_from(aligned_length).map_err(|_| BlockError::Io)?;
            source = &source[aligned_length..];
        }

        if !source.is_empty() {
            let mut block = [0_u8; BLOCK_SIZE];
            let index = start / BLOCK_SIZE_U64;

            self.device.read_blocks(index, &mut block)?;
            block[..source.len()].copy_from_slice(source);
            self.device.write_blocks(index, &block)?;
        }

        Ok(())
    }
}

impl Ext4Read for DeviceIo {
    fn read(
        &self,
        start_byte: u64,
        destination: &mut [u8],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.validate(start_byte, destination.len())?;

        let _io = self.io.lock();

        self.read_range(start_byte, destination).map_err(Into::into)
    }
}

impl Ext4Write for DeviceIo {
    fn write(&self, start_byte: u64, source: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.validate(start_byte, source.len())?;

        let _io = self.io.lock();

        self.write_range(start_byte, source).map_err(Into::into)
    }
}
