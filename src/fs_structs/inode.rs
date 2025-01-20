use std::{fs::File, io::Read, os::unix::fs::FileExt};

use thiserror::Error;

use crate::BlockIndex;

use super::super_block::{self, SuperBlock};

const FM_S_IXOTH: u16 = 0x1; // Others may execute
const FM_S_IWOTH: u16 = 0x2; // Others may write
const FM_S_IROTH: u16 = 0x4; // Others may read
const FM_S_IXGRP: u16 = 0x8; // Group members may execute
const FM_S_IWGRP: u16 = 0x10; // Group members may write
const FM_S_IRGRP: u16 = 0x20; // Group members may read
const FM_S_IXUSR: u16 = 0x40; // Owner may execute
const FM_S_IWUSR: u16 = 0x80; // Owner may write
const FM_S_IRUSR: u16 = 0x100; // Owner may read
const FM_S_ISVTX: u16 = 0x200; // Sticky bit
const FM_S_ISGID: u16 = 0x400; // Set GID
const FM_S_ISUID: u16 = 0x800; // Set UID

const FT_S_IFIFO: u16 = 0x1000; // FIFO
const FT_S_IFCHR: u16 = 0x2000; // Character device
const FT_S_IFDIR: u16 = 0x4000; // Directory
const FT_S_IFBLK: u16 = 0x6000; // Block device
const FT_S_IFREG: u16 = 0x8000; // Regular file
const FT_S_IFLNK: u16 = 0xA000; // Symbolic link
const FT_S_IFSOCK: u16 = 0xC000; // Socket

#[repr(C)]
pub struct InodeHeader {
    /// Bit mask of inode mode.
    mode: u16,
    parent: u32,
    name: [u8; 256],
    uid: u32,
    gid: u32,
    size: u64,
    atime: u32,
    ctime: u32,
    mtime: u32,
    dtime: u32,
    links_count: u16,
    blocks: u32,
    flags: u32,
    // contents: std::rc::Rc<[u32]>,
}

#[derive(Error, Debug)]
pub enum InodeError {
    #[error("File IO Error: {0}")]
    Io(#[from] std::io::Error),
}

impl InodeHeader {
    fn read(
        &mut self,
        file: File,
        block_index: BlockIndex,
        super_block: SuperBlock,
    ) -> Result<(), InodeError> {
        let mut buf = [0u8; std::mem::size_of::<InodeHeader>()];
        let _ = file.read_exact_at(&mut buf, block_index as u64 * super_block.block_size as u64);
        *self = unsafe { std::mem::transmute(buf) };
        Ok(())
    }
}
