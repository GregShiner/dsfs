use std::{fs::File, os::unix::fs::FileExt, path::PathBuf};

use thiserror::Error;

use crate::fs_structs::{block_table::BlockTable, super_block::SuperBlock};

pub struct Dsfs {
    block_file: File,
    mount_point: PathBuf,
    pub super_block: SuperBlock,
    pub blocks_in_group: u32,
    free_tables: Vec<BlockTable>,
}

#[derive(Error, Debug)]
enum DsfsError {}

impl Dsfs {
    // Loads an existing filesystem from a block file
    pub fn new(file_name: PathBuf, mount_point: PathBuf) -> std::io::Result<Self> {
        // Read superblock information
        let block_file = File::open(file_name).unwrap();

        let mut block_size_buf = [0 as u8; 4];
        let _ = block_file.read_exact_at(&mut block_size_buf, 0)?;
        // TODO: Check that this should not be u32::from_le_bytes() (im pretty sure this is right)
        let block_size = u32::from_be_bytes(block_size_buf);

        let mut num_blocks_buf = [0 as u8; 4];
        let _ = block_file.read_exact_at(&mut num_blocks_buf, 4)?;
        let num_blocks = u32::from_be_bytes(num_blocks_buf);

        let mut blocks_in_group_buf = [0 as u8; 4];
        let _ = block_file.read_exact_at(&mut blocks_in_group_buf, 8)?;
        let blocks_in_group = u32::from_be_bytes(blocks_in_group_buf);

        // Number of groups is ceil(num_blocks/blocks_in_group)
        let num_groups = num_blocks.div_ceil(blocks_in_group);
        let mut dsfs = Dsfs {
            block_file,
            mount_point,
            block_size,
            num_blocks,
            blocks_in_group,
            free_tables: vec![],
        };
        // For all groups, load a free table
        for group_index in 0..num_groups {
            dsfs.free_tables
                .push(BlockTable::from_fs(&mut dsfs.block_file, group_index).unwrap())
        }
        Ok(dsfs)
    }

    // fn create(file_name: PathBuf, mount_point: PathBuf, block_size: u32, ) -> std::io::Result<Self> {
}
