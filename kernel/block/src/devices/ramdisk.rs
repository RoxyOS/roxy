use alloc::boxed::Box;

use roxy_utils::Lock;

use crate::{BlockDevice, BlockError};

pub const LOGICAL_BLOCK_SIZE: usize = 512;

pub struct RamDisk {
    data: Lock<Box<[u8]>>,
}

impl RamDisk {
    pub fn new(source: &[u8]) -> Result<Self, BlockError> {
        if source.is_empty() || !source.len().is_multiple_of(LOGICAL_BLOCK_SIZE) {
            return Err(BlockError::Misaligned);
        }

        Ok(Self {
            data: Lock::new(source.into()),
        })
    }

    fn byte_range(
        &self,
        start: u64,
        byte_len: usize,
    ) -> Result<core::ops::Range<usize>, BlockError> {
        if !byte_len.is_multiple_of(LOGICAL_BLOCK_SIZE) {
            return Err(BlockError::Misaligned);
        }

        let start = usize::try_from(start)
            .ok()
            .and_then(|index| index.checked_mul(LOGICAL_BLOCK_SIZE))
            .ok_or(BlockError::OutOfBounds)?;
        let end = start.checked_add(byte_len).ok_or(BlockError::OutOfBounds)?;
        if end > self.data.lock().len() {
            return Err(BlockError::OutOfBounds);
        }

        Ok(start..end)
    }
}

impl BlockDevice for RamDisk {
    fn block_size(&self) -> usize {
        LOGICAL_BLOCK_SIZE
    }

    fn block_count(&self) -> u64 {
        u64::try_from(self.data.lock().len() / LOGICAL_BLOCK_SIZE)
            .expect("RAM disk block count must fit in u64")
    }

    fn read_blocks(&self, start: u64, destination: &mut [u8]) -> Result<(), BlockError> {
        let range = self.byte_range(start, destination.len())?;
        let data = self.data.lock();

        destination.copy_from_slice(&data[range]);
        Ok(())
    }

    fn write_blocks(&self, start: u64, source: &[u8]) -> Result<(), BlockError> {
        let range = self.byte_range(start, source.len())?;
        let mut data = self.data.lock();

        data[range].copy_from_slice(source);
        Ok(())
    }

    fn flush(&self) -> Result<(), BlockError> {
        Ok(())
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::{BlockDevice, BlockError, LOGICAL_BLOCK_SIZE, RamDisk};

    roxy_test::kernel_test!(
        "roxy-block::ram-disk-validates-io",
        ram_disk_validates_io,
        {
            let mut source = [0_u8; LOGICAL_BLOCK_SIZE * 2];
            source[LOGICAL_BLOCK_SIZE] = 7;
            let disk = RamDisk::new(&source).unwrap();
            source[LOGICAL_BLOCK_SIZE] = 9;
            assert_eq!(source[LOGICAL_BLOCK_SIZE], 9);

            let mut block = [0_u8; LOGICAL_BLOCK_SIZE];
            disk.read_blocks(1, &mut block).unwrap();
            assert_eq!(block[0], 7);
            assert_eq!(
                disk.read_blocks(0, &mut block[..1]),
                Err(BlockError::Misaligned)
            );
            assert_eq!(
                disk.read_blocks(2, &mut block),
                Err(BlockError::OutOfBounds)
            );

            block[0] = 3;
            disk.write_blocks(0, &block).unwrap();
            disk.read_blocks(0, &mut block).unwrap();
            assert_eq!(block[0], 3);
            assert_eq!(disk.block_size(), LOGICAL_BLOCK_SIZE);
            assert_eq!(disk.block_count(), 2);
            disk.flush().unwrap();
        }
    );
}
