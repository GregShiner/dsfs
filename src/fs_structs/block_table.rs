use std::os::unix::fs::FileExt;
use thiserror::Error;

use crate::{dsfs::Dsfs, GroupIndex};

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

// NOTE: EXTREMELY IMPORTANT!!!! Do not change this type without ensuring that TryFrom<u8> for
// BlockType is updated!! Not updating this trait impl can and will lead to UB
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

impl TryFrom<u8> for BlockType {
    type Error = ();

    fn try_from(byte: u8) -> Result<Self, <BlockType as TryFrom<u8>>::Error> {
        if byte > 0x6 {
            Err(())
        } else {
            // This should be safe because the byte is checked to be defined by the enum by the if
            // statement. BlockType uses the same representation as a u8 as well.
            Ok(unsafe { std::mem::transmute(byte) })
        }
    }
}

impl BlockTable {
    // There HAS to be a better way to do this
    fn table_as_bytes(&self) -> Vec<u8> {
        self.table.iter().map(|&e| e as u8).collect()
    }

    fn table_from_bytes(
        bytes: Vec<u8>,
    ) -> Result<Vec<BlockType>, <BlockType as TryFrom<u8>>::Error> {
        bytes
            .iter()
            .map(|&e| e.try_into().or_else(|_| Err(())))
            .collect()
    }
    // Creates a new block table, writes it to the disk, and returns it
    fn create_and_init(
        dsfs: &Dsfs,
        // block_file: &mut File,
        group_index: GroupIndex,
        // blocks_in_group: u32,
    ) -> Result<Self, BlockTableError> {
        // Creates an array of
        let mut table: Vec<BlockType> =
            vec![BlockType::Free; dsfs.blocks_in_group.try_into().unwrap()];
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
        match block_table.write_table(dsfs) {
            Ok(_) => Ok(block_table),
            Err(err) => Err(err),
        }
    }

    // Creates a BlockTable from an existing ft on the fs
    pub fn from_fs(
        dsfs: &Dsfs,
        // block_file: &mut File,
        group_index: GroupIndex,
        // blocks_in_group: u32,
    ) -> Result<BlockTable, BlockTableError> {
        let table: Vec<BlockType> = vec![BlockType::Free; dsfs.blocks_in_group.try_into().unwrap()];
        let mut block_table = BlockTable { table, group_index };
        match block_table.read_table(dsfs) {
            Ok(_) => Ok(block_table),
            Err(err) => Err(err),
        }
    }

    // Writes table state from memory to disk
    fn write_table(
        &self,
        dsfs: &Dsfs,
        // block_file: &mut File,
        // super_block: SuperBlock,
    ) -> Result<(), BlockTableError> {
        let block_index = match self.group_index {
            0 => 1,
            // block_size is being used here to get the blocks in a group
            // Multiplying the blocks in a group with the group index gets the block index of the
            // groups block table.
            _ => dsfs.block_size * self.group_index,
        };
        match dsfs.block_file.write_all_at(
            self.table_as_bytes().as_slice(),
            (block_index * dsfs.block_size).into(),
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(BlockTableError::FileError),
        }
    }

    // Reads table state from disk to memory
    fn read_table(
        &mut self,
        dsfs: &Dsfs,
        // block_file: &File,
        // blocks_in_group: u32,
    ) -> Result<(), BlockTableError> {
        let block_index = match self.group_index {
            0 => 1,
            _ => dsfs.blocks_in_group * self.group_index,
        };
        let mut table: Vec<u8> = vec![0; dsfs.blocks_in_group.try_into().unwrap()];
        match dsfs
            .block_file
            .read_exact_at(&mut table, (block_index * dsfs.block_size).into())
        {
            Ok(_) => {
                self.table = Self::table_from_bytes(table).unwrap();
                Ok(())
            }
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
        self.table[block_in_group_index as usize] = value;
        Ok(())
    }

    // NOTE: This only gets the type from the table in memory. You must call read_table() before
    // this function to get any potentially changed data. This is seperated so you can make
    // multiple consecutive calls to this function with only a single IO operation.
    fn get_type(
        &mut self,
        block_in_group_index: u32, // Ditto
        fs: &Dsfs,
    ) -> Result<BlockType, BlockTableError> {
        // TODO: Check this condition (maybe off by 1)
        if block_in_group_index >= fs.blocks_in_group {
            return Err(BlockTableError::OutOfBounds(
                block_in_group_index,
                fs.blocks_in_group,
            ));
        }
        Ok(self.table[block_in_group_index as usize])
    }
}
