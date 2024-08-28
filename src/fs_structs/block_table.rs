use std::{fs::File, os::unix::fs::FileExt};
use thiserror::Error;

use crate::{dsfs::Dsfs, GroupIndex, BLOCKS_IN_GROUP, BLOCK_SIZE};

pub struct BlockTable {
    table: Vec<BlockType>,
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

#[repr(u8)]
#[derive(Copy, Clone)]
enum BlockType {
    Free = 0x0,
    SuperBlock = 0x1,
    BlockTable = 0x2,
    Inode = 0x3,
    Data = 0x4,
    Error = 0x5,
    IndirectionTable = 0x6,
}

impl Into<u8> for BlockType {
    fn into(self) -> u8 {
        self as u8
    }
}

impl BlockTable {
    // Creates a new block table, writes it to the disk, and returns it
    fn create_and_init(
        block_file: &mut File,
        group_index: GroupIndex,
        blocks_in_group: u32,
    ) -> Result<Self, BlockTableError> {
        // Creates an array of
        let mut table: Vec<BlockType> = vec![BlockType::Free; blocks_in_group.try_into().unwrap()];
        // If the first group, the first block is the superblock and the second is the block table.
        // Else, the first block is the block table.
        match group_index {
            0 => {
                table[0] = BlockType::SuperBlock;
                table[1] = BlockType::BlockTable;
            }
            _ => {
                table[0] = BlockType::BlockTable;
            }
        };
        let block_table = BlockTable { table, group_index };
        match block_table.write_table(block_file) {
            Ok(_) => Ok(block_table),
            Err(err) => Err(err),
        }
    }

    // Creates a BlockTable from an existing ft on the fs
    pub fn from_fs(
        block_file: &mut File,
        group_index: GroupIndex,
        blocks_in_group: u32,
    ) -> Result<BlockTable, BlockTableError> {
        let table: Vec<BlockType> = vec![BlockType::Free; blocks_in_group.try_into().unwrap()];
        let mut block_table = BlockTable { table, group_index };
        match block_table.read_table(block_file) {
            Ok(_) => Ok(block_table),
            Err(err) => Err(err),
        }
    }

    /*
    let blocks_in_group: u32 = 4;
    let table: Vec<BlockType> = vec![BlockType::BlockTable; blocks_in_group.try_into().unwrap()];
    println!("{table:?}");
    let writeable: Vec<u8> = unsafe {
        std::mem::transmute::<&[BlockType], &[u8]>(table.as_slice())
    }.into();
    let mut arr: Arc<[i32]> = Arc::new([1, 2, 3]);
    arr[1] = 5;
    println!("{arr:?}");
    println!("{writeable:?}");
    */
    // Writes table state from memory to disk
    fn write_table(&self, block_file: &mut File) -> Result<(), BlockTableError> {
        let block_index = match self.group_index {
            0 => 1,
            _ => BLOCKS_IN_GROUP * self.group_index,
        };
        match block_file.write_all_at(
            self.table.into::<Vec<u8>>().into(),
            (block_index * BLOCK_SIZE).into(),
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(BlockTableError::FileError),
        }
    }

    // Reads table state from disk to memory
    fn read_table(&mut self, block_file: &File) -> Result<(), BlockTableError> {
        let block_index = match self.group_index {
            0 => 1,
            _ => BLOCKS_IN_GROUP * self.group_index,
        };
        match block_file.read_exact_at(&mut self.table, (block_index * BLOCK_SIZE).into()) {
            Ok(_) => Ok(()),
            Err(_) => Err(BlockTableError::FileError),
        }
    }

    // NOTE: This function only updates the table in memory. You must call write_table() at some
    // point to actually write the changes. This is seperated so multiple type changes can be
    // written to the disk atomically, and to reduce the number of IO operations.
    fn set_type(
        &mut self,
        block_in_group_index: u32, // This is the index of the byte inside the current block table. This is NOT
        // the same as the block index. It will be block_index % BLOCKS_IN_GROUP b/c it is the
        // index of the block within a group
        fs: &Dsfs,
        value: BlockType,
    ) -> Result<(), BlockTableError> {
        // TODO: Check this condition (maybe off by 1)
        if block_in_group_index >= fs.blocks_in_group {
            return Err(BlockTableError::OutOfBounds(
                block_in_group_index,
                fs.blocks_in_group,
            ));
        }
        self.table[block_in_group_index] = value;
        Ok(())
    }

    // NOTE: This only gets the type from the table in memory. You must call read_table() before
    // this function to get any potentially changed data. This is seperated so you can make
    // multiple consecutive calls to this function with only a single IO operation.
    fn get_type(
        &mut self,
        block_in_group_index: u32, // Ditto
        block_file: &mut File,
        fs: &Dsfs,
    ) -> Result<bool, BlockTableError> {
        // TODO: Check this condition (maybe off by 1)
        if block_in_group_index >= fs.blocks_in_group {
            return Err(BlockTableError::OutOfBounds(
                block_in_group_index,
                fs.blocks_in_group,
            ));
        }
        Ok(self.table[block_in_group_index])
    }
}
