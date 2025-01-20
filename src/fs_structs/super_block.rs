use thiserror::Error;

use super::FsStruct;

#[derive(Debug)]
#[repr(C)]
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

    pub fn empty() -> Self {
        SuperBlock {
            block_size: 0,
            num_blocks: 0,
        }
    }
}

impl FsStruct<SuperBlockError> for SuperBlock {}
