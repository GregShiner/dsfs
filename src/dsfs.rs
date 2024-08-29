use std::{error, fs::File, os::unix::fs::FileExt, path::PathBuf};

use thiserror::Error;

use crate::fs_structs::{
    block_table::{BlockTable, BlockTableError},
    super_block::SuperBlock,
};

pub struct Dsfs {
    pub block_file: File,
    mount_point: PathBuf,
    pub block_size: u32,
    pub num_blocks: u32,
    /// Always equal to block_size because it is limited by the number of entries in a block table
    pub blocks_in_group: u32,
    block_table: Vec<BlockTable>,
}

#[derive(Error, Debug)]
enum DsfsError {
    #[error("File IO Error")]
    IoError(#[from] std::io::Error),
    #[error("Block Table Error: {0}")]
    BlockTableError(#[from] BlockTableError),
}

impl Dsfs {
    // Loads an existing filesystem from a block file
    pub fn load(file_name: PathBuf, mount_point: PathBuf) -> Result<Self, DsfsError> {
        // Read superblock information
        let block_file = File::open(file_name)?;

        let SuperBlock {
            block_size,
            num_blocks,
        } = SuperBlock::new(&block_file)?;

        let blocks_in_group = block_size;

        // Number of groups is ceil(num_blocks/blocks_in_group)
        let num_groups = num_blocks.div_ceil(blocks_in_group);
        let mut dsfs = Dsfs {
            block_file,
            mount_point,
            block_size,
            num_blocks,
            blocks_in_group,
            block_table: vec![],
        };
        // For all groups, load a free table
        for group_index in 0..num_groups {
            dsfs.block_table
                .push(BlockTable::from_fs(&dsfs, group_index)?)
        }
        Ok(dsfs)
    }

    fn create(
        file_name: PathBuf,
        mount_point: PathBuf,
        block_size: u32,
    ) -> Result<Self, DsfsError> {
        // Read superblock information
        let block_file = File::open(file_name)?;

        let mut blocks_in_group_buf = [0 as u8; 4];
        let _ = block_file.read_exact_at(&mut blocks_in_group_buf, 8)?;
        let blocks_in_group = u32::from_be_bytes(blocks_in_group_buf);

        let SuperBlock {
            block_size,
            num_blocks,
        } = SuperBlock::new(&block_file)?;

        // Number of groups is ceil(num_blocks/blocks_in_group)
        let num_groups = num_blocks.div_ceil(blocks_in_group);
        let mut dsfs = Dsfs {
            block_file,
            mount_point,
            block_size,
            num_blocks,
            blocks_in_group,
            block_table: vec![],
        };
        // For all groups, load a free table
        for group_index in 0..num_groups {
            dsfs.block_table
                .push(BlockTable::from_fs(&dsfs, group_index)?)
        }
        Ok(dsfs)
    }
}
