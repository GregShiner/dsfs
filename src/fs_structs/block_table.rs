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
    #[error("Invalid block type byte {0}")]
    InvalidBlockType(u8),
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
    type Error = BlockTableError;

    fn try_from(byte: u8) -> Result<Self, <BlockType as TryFrom<u8>>::Error> {
        if byte > 0x6 {
            Err(BlockTableError::InvalidBlockType(byte))
        } else {
            // This should be safe because the byte is checked to be defined by the enum by the if
            // statement. BlockType uses the same representation as a u8 as well.
            Ok(unsafe { std::mem::transmute(byte) })
        }
    }
}

impl BlockTable {
    /// There HAS to be a better way to do this
    /// Returns the table as a vector of bytes
    fn table_as_bytes(&self) -> Vec<u8> {
        self.table.iter().map(|&e| e as u8).collect()
    }

    /// Converts a vector of bytes from the block table to a vector of block types
    /// Errors if a byte read does not fit a BlockType variant
    fn table_from_bytes(bytes: Vec<u8>) -> Result<Vec<BlockType>, BlockTableError> {
        // Loops through the bytes and casts each one to a BlockType
        bytes
            .iter()
            .map(|&e| {
                e.try_into()
                    // If it fails to convert, propogates the error
                    .or_else(|_| Err(BlockTableError::InvalidBlockType(e)))
            })
            .collect()
    }

    /// Initializes a new vector of BlockType::Free
    #[inline]
    fn new_table(size: u32) -> Result<Vec<BlockType>, BlockTableError> {
        Ok(vec![
            BlockType::Free;
            // This ugly ass block converts the dsfs.blocks_in_group which is a u32 to a usize and
            // errors if you cant
            size.try_into().or_else(|_| Err(
                BlockTableError::TypeCastError(
                    std::any::type_name_of_val(&size),
                    std::any::type_name::<usize>()
                )
            ))?
        ])
    }

    /// Creates a new block table, writes it to the disk, and returns it
    fn create_and_init(dsfs: &Dsfs, group_index: GroupIndex) -> Result<Self, BlockTableError> {
        let mut table = Self::new_table(dsfs.blocks_in_group)?;
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
        // Construct the block table
        let block_table = BlockTable { table, group_index };
        // Write it to the disk
        match block_table.write_table(dsfs) {
            Ok(_) => Ok(block_table),
            Err(err) => Err(err),
        }
    }

    /// Creates a BlockTable from an existing block table on the fs
    pub fn from_fs(dsfs: &Dsfs, group_index: GroupIndex) -> Result<BlockTable, BlockTableError> {
        let table = Self::new_table(dsfs.blocks_in_group)?;
        let mut block_table = BlockTable { table, group_index };
        match block_table.read_table(dsfs) {
            Ok(_) => Ok(block_table),
            Err(err) => Err(err),
        }
    }

    /// Writes table state from memory to disk
    fn write_table(&self, dsfs: &Dsfs) -> Result<(), BlockTableError> {
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

    /// Reads table state from disk to memory
    fn read_table(&mut self, dsfs: &Dsfs) -> Result<(), BlockTableError> {
        let block_index = match self.group_index {
            0 => 1,
            _ => dsfs.blocks_in_group * self.group_index,
        };
        let mut table: Vec<u8> = vec![
            0;
            dsfs.blocks_in_group.try_into().or_else(|_| Err(
                BlockTableError::TypeCastError(
                    std::any::type_name_of_val(&dsfs.blocks_in_group),
                    std::any::type_name::<usize>()
                )
            ))?
        ];
        match dsfs
            .block_file
            .read_exact_at(&mut table, (block_index * dsfs.block_size).into())
        {
            Ok(_) => {
                self.table = Self::table_from_bytes(table)?;
                Ok(())
            }
            Err(_) => Err(BlockTableError::FileError),
        }
    }

    /// Sets the type of a block in memory
    /// NOTE: This function only updates the table in memory. You must call write_table() at some
    /// point to actually write the changes. This is seperated so multiple type changes can be
    /// written to the disk atomically, and to reduce the number of IO operations.
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

    /// Gets a type of a block in memory
    /// NOTE: This only gets the type from the table in memory. You must call read_table() before
    /// this function to get any potentially changed data. This is seperated so you can make
    /// multiple consecutive calls to this function with only a single IO operation.
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
