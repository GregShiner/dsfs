use clap::{crate_version, Arg, ArgAction, Command};
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use libc::ENOENT;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};
use thiserror::Error;

type BlockIndex = u32;
type GroupIndex = u32;

const BLOCK_SIZE: u32 = 4096; // 4KiB
const NUM_BLOCKS: u32 = 1024; // 1024 Blocks = 4.0MiB ~= 4.2MB
const BLOCKS_IN_GROUP: u32 = BLOCK_SIZE * 8; // Number of blocks in a group. This is limited by the
                                             // number of bits in a free table, which is a single full block

const TTL: Duration = Duration::from_secs(1); // 1 second

const HELLO_DIR_ATTR: FileAttr = FileAttr {
    ino: 1,
    size: 0,
    blocks: 0,
    atime: UNIX_EPOCH, // 1970-01-01 00:00:00
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::Directory,
    perm: 0o755,
    nlink: 2,
    uid: 501,
    gid: 20,
    rdev: 0,
    flags: 0,
    blksize: 512,
};

const HELLO_TXT_CONTENT: &str = "Hello World!\n";

const HELLO_TXT_ATTR: FileAttr = FileAttr {
    ino: 2,
    size: 13,
    blocks: 1,
    atime: UNIX_EPOCH, // 1970-01-01 00:00:00
    mtime: UNIX_EPOCH,
    ctime: UNIX_EPOCH,
    crtime: UNIX_EPOCH,
    kind: FileType::RegularFile,
    perm: 0o644,
    nlink: 1,
    uid: 501,
    gid: 20,
    rdev: 0,
    flags: 0,
    blksize: 512,
};

struct Dsfs {
    block_file: File,
    mount_point: PathBuf,
    block_size: u32,
    num_blocks: u32,
    blocks_in_group: u32,
    free_tables: Vec<FreeTable>,
}

struct FreeTable {
    table: [u8; BLOCKS_IN_GROUP as usize / 8],
    group_index: GroupIndex,
}

#[derive(Error, Debug)]
enum FreeTableError {
    #[error("The bit index is out of bounds. Bit index provided: {0}, Max bit index: {1}")]
    OutOfBounds(u32, u32),
    #[error("File error")]
    FileError,
    #[error("Type cast error: From {0} to {1}")]
    TypeCastError(&'static str, &'static str),
}

impl FreeTable {
    // Creates a new free table, writes it to the disk, and returns it
    fn create_and_init(
        block_file: &mut File,
        group_index: GroupIndex,
    ) -> Result<Self, FreeTableError> {
        let block_index = match group_index {
            0 => 1,
            _ => BLOCKS_IN_GROUP * group_index,
        };
        let mut table = [0 as u8; BLOCKS_IN_GROUP as usize / 8];
        // TODO: Set initial bits
        let free_table = FreeTable { table, group_index };
        match free_table.update_file(block_file) {
            Ok(_) => Ok(free_table),
            Err(err) => Err(err),
        }
    }

    // Creates a FileTable from an existing ft on the fs
    fn from_fs(
        block_file: &mut File,
        group_index: GroupIndex,
    ) -> Result<FreeTable, FreeTableError> {
        let table = [0 as u8; BLOCKS_IN_GROUP as usize / 8];
        let mut free_table = FreeTable { table, group_index };
        match free_table.update_table(block_file) {
            Ok(_) => Ok(free_table),
            Err(err) => Err(err),
        }
    }

    fn update_file(&self, block_file: &mut File) -> Result<(), FreeTableError> {
        let block_index = match self.group_index {
            0 => 1,
            _ => BLOCKS_IN_GROUP * self.group_index,
        };
        match block_file.write_all_at(&self.table, (block_index * BLOCK_SIZE).into()) {
            Ok(_) => Ok(()),
            Err(_) => Err(FreeTableError::FileError),
        }
    }

    fn update_table(&mut self, block_file: &File) -> Result<(), FreeTableError> {
        let block_index = match self.group_index {
            0 => 1,
            _ => BLOCKS_IN_GROUP * self.group_index,
        };
        match block_file.read_exact_at(&mut self.table, (block_index * BLOCK_SIZE).into()) {
            Ok(_) => Ok(()),
            Err(_) => Err(FreeTableError::FileError),
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
    ) -> Result<(), FreeTableError> {
        // TODO: Check this condition (maybe off by 1)
        if bit_index >= fs.blocks_in_group {
            return Err(FreeTableError::OutOfBounds(bit_index, fs.blocks_in_group));
        }
        // Index of the [u8]
        let arr_index: usize = match (bit_index / 8).try_into() {
            Ok(ok) => ok,
            Err(_) => {
                return Err(FreeTableError::TypeCastError(
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
    ) -> Result<bool, FreeTableError> {
        // TODO: Check this condition (maybe off by 1)
        if bit_index >= fs.blocks_in_group {
            return Err(FreeTableError::OutOfBounds(bit_index, fs.blocks_in_group));
        }
        // Index of the [u8]
        let arr_index: usize = match (bit_index / 8).try_into() {
            Ok(ok) => ok,
            Err(_) => {
                return Err(FreeTableError::TypeCastError(
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

#[derive(Error, Debug)]
enum DsfsError {}

impl Dsfs {
    // Loads an existing filesystem from a block file
    fn new(file_name: PathBuf, mount_point: PathBuf) -> std::io::Result<Self> {
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
                .push(FreeTable::from_fs(&mut dsfs.block_file, group_index).unwrap())
        }
        Ok(dsfs)
    }

    // fn create(file_name: PathBuf, mount_point: PathBuf, block_size: u32, ) -> std::io::Result<Self> {
}

impl Filesystem for Dsfs {
    fn init(
        &mut self,
        _req: &Request<'_>,
        _config: &mut fuser::KernelConfig,
    ) -> Result<(), libc::c_int> {
        println!("Successfully Mounted");
        Ok(())
    }
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if parent == 1 && name.to_str() == Some("hello.txt") {
            reply.entry(&TTL, &HELLO_TXT_ATTR, 0);
        } else {
            reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        match ino {
            1 => reply.attr(&TTL, &HELLO_DIR_ATTR),
            2 => reply.attr(&TTL, &HELLO_TXT_ATTR),
            _ => reply.error(ENOENT),
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        _size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        if ino == 2 {
            reply.data(&HELLO_TXT_CONTENT.as_bytes()[offset as usize..]);
        } else {
            reply.error(ENOENT);
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        if ino != 1 {
            reply.error(ENOENT);
            return;
        }

        let entries = vec![
            (1, FileType::Directory, "."),
            (1, FileType::Directory, ".."),
            (2, FileType::RegularFile, "hello.txt"),
        ];

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // i + 1 means the index of the next entry
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
    }
}

fn main() {
    let matches = Command::new("dsfs")
        .version(crate_version!())
        .author("Gregory Shiner")
        .arg(
            Arg::new("MOUNT_POINT")
                .required(false)
                .index(1)
                .help("Act as a client, and mount FUSE at given path")
                .default_value("./mnt"),
        )
        .arg(
            Arg::new("DEVICE_FILE")
                .required(false)
                .index(2)
                .help("Mount a dsfs filesystem stored in a specific block device file")
                .default_value("dsfs.img"),
        )
        .arg(
            Arg::new("create_fs")
                .required(false)
                .short('c')
                .long("create-fs")
                .help("initializes a new filesystem at the given device file"),
        )
        .arg(
            Arg::new("no-auto-unmount")
                .long("no-auto-unmount")
                .action(ArgAction::SetFalse)
                .help("Automatically unmount on process exit"),
        )
        .arg(
            Arg::new("allow-root")
                .long("allow-root")
                .action(ArgAction::SetTrue)
                .help("Allow root user to access filesystem"),
        )
        .get_matches();
    env_logger::init();
    let mount_point = matches.get_one::<PathBuf>("MOUNT_POINT").unwrap();
    let fs_filename = matches.get_one::<PathBuf>("DEVICE_FILE").unwrap();
    let mut options = vec![MountOption::RW, MountOption::FSName("dsfs".to_string())];
    if matches.get_flag("no-auto-unmount") {
        options.push(MountOption::AutoUnmount);
    }
    if matches.get_flag("allow-root") {
        options.push(MountOption::AllowRoot);
    }
    // println!("Mounting {} on {}", fs_filename.into(), mount_point.into());
    let mut dsfs = Dsfs {
        block_file: File::open(fs_filename).unwrap(),
        mount_point: mount_point.to_path_buf(),
        block_size: BLOCK_SIZE,
        num_blocks: NUM_BLOCKS,
        blocks_in_group: BLOCKS_IN_GROUP,
        free_tables: vec![],
    };
    dsfs.free_tables
        .push(FreeTable::create_and_init(&mut dsfs.block_file, 0).unwrap());
    fuser::mount2(dsfs, mount_point, &options).unwrap();
}
