use std::{fs::File, os::unix::fs::FileExt};

pub struct SuperBlock {
    pub block_size: u32,
    pub num_blocks: u32,
}

impl SuperBlock {
    // Loads an existing filesystem from a block file
    pub fn new(block_file: &File) -> std::io::Result<Self> {
        let mut block_size_buf = [0 as u8; 4];
        let _ = block_file.read_exact_at(&mut block_size_buf, 0)?;
        // TODO: Check that this should not be u32::from_le_bytes() (im pretty sure this is right)
        let block_size = u32::from_be_bytes(block_size_buf);

        let mut num_blocks_buf = [0 as u8; 4];
        let _ = block_file.read_exact_at(&mut num_blocks_buf, 4)?;
        let num_blocks = u32::from_be_bytes(num_blocks_buf);

        Ok(SuperBlock {
            block_size,
            num_blocks,
        })
    }
}
