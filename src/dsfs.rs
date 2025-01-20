use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
};

use fuser::Filesystem;
use thiserror::Error;

use crate::fs_structs::{
    block_table::{BlockTable, BlockTableError},
    generic::FsStruct,
    super_block::{SuperBlock, SuperBlockError},
};

#[derive(Debug)]
pub struct Dsfs {
    pub block_file: File,
    mount_point: PathBuf,
    pub super_block: SuperBlock,
    /// Always equal to block_size because it is limited by the number of entries in a block table
    pub blocks_in_group: u32,
    block_tables: Vec<BlockTable>,
}

#[derive(Error, Debug)]
pub enum DsfsError {
    #[error("File IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Block Table Error: {0}")]
    BlockTable(#[from] BlockTableError),
    #[error("Super Block Error: {0}")]
    SuperBlock(#[from] SuperBlockError),
}

impl Filesystem for Dsfs {}

impl Dsfs {
    // Loads an existing filesystem from a block file
    pub fn load(file_name: PathBuf, mount_point: PathBuf) -> Result<Self, DsfsError> {
        // Read superblock information
        let block_file = OpenOptions::new().read(true).write(true).open(file_name)?;

        let mut super_block = SuperBlock::empty();
        super_block.read(&block_file, 0)?;

        let blocks_in_group = super_block.block_size;

        // Number of groups is ceil(num_blocks/blocks_in_group)
        let num_groups = super_block.num_blocks.div_ceil(blocks_in_group);
        let mut dsfs = Dsfs {
            block_file,
            mount_point,
            super_block,
            blocks_in_group,
            block_tables: vec![],
        };
        // For all groups, load a block table
        for group_index in 0..num_groups {
            dsfs.block_tables
                .push(BlockTable::from_fs(&dsfs, group_index)?)
        }
        Ok(dsfs)
    }

    pub fn create_and_write(
        file_name: PathBuf,
        mount_point: PathBuf,
        block_size: u32,
    ) -> Result<Self, DsfsError> {
        let block_file = OpenOptions::new().read(true).write(true).open(file_name)?;

        let blocks_in_group = block_size; // These are always equal
        let super_block = SuperBlock::new(block_size, 3u32); // 3 because there are always 3 blocks
                                                             // when dsfs is first created: super block, first block table, and root dir inode
        super_block.write(&block_file, 0);

        let mut dsfs = Dsfs {
            block_file,
            mount_point,
            super_block,
            blocks_in_group,
            block_tables: vec![],
        };
        dsfs.block_tables
            .push(BlockTable::create_and_write(&dsfs, 0)?);
        Ok(dsfs)
    }
}
