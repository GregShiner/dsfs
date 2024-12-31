use std::{fs::File, os::unix::fs::FileExt};

use thiserror::Error;

#[derive(Debug)]
pub struct SuperBlock {
    pub block_size: u32,
    pub num_blocks: u32,
}

#[derive(Error, Debug)]
pub enum SuperBlockError {
    #[error("File IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Num blocks set in super block is 0")]
    ZeroBlocks,
}

impl SuperBlock {
    pub fn new(block_size: u32, num_blocks: u32) -> Self {
        SuperBlock {
            block_size,
            num_blocks,
        }
    }

    /// Loads an existing filesystem from a block file
    pub fn read(block_file: &File) -> Result<SuperBlock, SuperBlockError> {
        let mut block_size_buf = [0u8; 4];
        block_file.read_exact_at(&mut block_size_buf, 0)?;
        // TODO: Check that this should not be u32::from_be_bytes() (im pretty sure this is right)
        let block_size = u32::from_le_bytes(block_size_buf);

        let mut num_blocks_buf = [0u8; 4];
        block_file.read_exact_at(&mut num_blocks_buf, 4)?;
        let num_blocks = u32::from_le_bytes(num_blocks_buf);

        if num_blocks == 0 {
            return Err(SuperBlockError::ZeroBlocks);
        }

        Ok(SuperBlock {
            block_size,
            num_blocks,
        })
    }

    /// Update the block file with the current contents
    pub fn write(&self, block_file: &File) -> Result<(), SuperBlockError> {
        block_file.write_all_at(&u32::to_le_bytes(self.block_size), 0)?;
        block_file.write_all_at(&u32::to_le_bytes(self.num_blocks), 4)?;
        Ok(())
    }
}
