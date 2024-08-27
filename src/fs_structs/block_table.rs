use std::{fs::File, os::unix::fs::FileExt};
use thiserror::Error;

use crate::{dsfs::Dsfs, GroupIndex, BLOCKS_IN_GROUP, BLOCK_SIZE};

pub struct BlockTable {
    table: Vec<u8>,
    group_index: GroupIndex,
}

#[derive(Error, Debug)]
pub enum BlockTableError {
    #[error("The bit index is out of bounds. Bit index provided: {0}, Max bit index: {1}")]
    OutOfBounds(u32, u32),
    #[error("File error")]
    FileError,
    #[error("Type cast error: From {0} to {1}")]
    TypeCastError(&'static str, &'static str),
}

impl BlockTable {
    // Creates a new free table, writes it to the disk, and returns it
    fn create_and_init(
        block_file: &mut File,
        group_index: GroupIndex,
    ) -> Result<Self, BlockTableError> {
        let table: Vec<u8> = [0 as u8; BLOCKS_IN_GROUP as usize / 8].into();
        // TODO: Set initial bytes
        let free_table = BlockTable { table, group_index };
        match free_table.update_file(block_file) {
            Ok(_) => Ok(free_table),
            Err(err) => Err(err),
        }
    }

    // Creates a FileTable from an existing ft on the fs
    pub fn from_fs(
        block_file: &mut File,
        group_index: GroupIndex,
    ) -> Result<BlockTable, BlockTableError> {
        let table: Vec<u8> = [0 as u8; BLOCKS_IN_GROUP as usize / 8].into();
        let mut free_table = BlockTable { table, group_index };
        match free_table.update_table(block_file) {
            Ok(_) => Ok(free_table),
            Err(err) => Err(err),
        }
    }

    fn update_file(&self, block_file: &mut File) -> Result<(), BlockTableError> {
        let block_index = match self.group_index {
            0 => 1,
            _ => BLOCKS_IN_GROUP * self.group_index,
        };
        match block_file.write_all_at(&self.table, (block_index * BLOCK_SIZE).into()) {
            Ok(_) => Ok(()),
            Err(_) => Err(BlockTableError::FileError),
        }
    }

    fn update_table(&mut self, block_file: &File) -> Result<(), BlockTableError> {
        let block_index = match self.group_index {
            0 => 1,
            _ => BLOCKS_IN_GROUP * self.group_index,
        };
        match block_file.read_exact_at(&mut self.table, (block_index * BLOCK_SIZE).into()) {
            Ok(_) => Ok(()),
            Err(_) => Err(BlockTableError::FileError),
        }
    }

    fn set_bit(
        &mut self,
        block_file: &mut File,
        bit_index: u32, // This is the index of the bit inside the current free table. This is NOT
        // the same as the block index. It will be block_index % BLOCKS_IN_GROUP b/c it is the
        // index of the block within a group
        fs: &Dsfs,
        value: bool,
    ) -> Result<(), BlockTableError> {
        // TODO: Check this condition (maybe off by 1)
        if bit_index >= fs.blocks_in_group {
            return Err(BlockTableError::OutOfBounds(bit_index, fs.blocks_in_group));
        }
        // Index of the [u8]
        let arr_index: usize = match (bit_index / 8).try_into() {
            Ok(ok) => ok,
            Err(_) => {
                return Err(BlockTableError::TypeCastError(
                    std::any::type_name::<u32>(),
                    std::any::type_name::<u32>(),
                ))
            }
        };
        let u8_index = 7 - (bit_index % 8); // Index of bit inside of the u8
        match value {
            true => self.table[arr_index] |= 0b1 << u8_index,
            false => self.table[arr_index] &= 0b0 << u8_index,
        };
        Ok(())
    }

    fn get_bit(
        &mut self,
        block_file: &mut File,
        bit_index: u32, // Ditto
        fs: &Dsfs,
    ) -> Result<bool, BlockTableError> {
        // TODO: Check this condition (maybe off by 1)
        if bit_index >= fs.blocks_in_group {
            return Err(BlockTableError::OutOfBounds(bit_index, fs.blocks_in_group));
        }
        // Index of the [u8]
        let arr_index: usize = match (bit_index / 8).try_into() {
            Ok(ok) => ok,
            Err(_) => {
                return Err(BlockTableError::TypeCastError(
                    std::any::type_name::<u32>(),
                    std::any::type_name::<u32>(),
                ))
            }
        };
        let u8_index = 7 - (bit_index % 8); // Index of bit inside of the u8

        // Theres gotta be a better way to do this
        Ok(if self.table[arr_index] >> u8_index == 1 {
            true
        } else {
            false
        })
    }
}
