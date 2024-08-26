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
use std::time::{Duration, UNIX_EPOCH};
use thiserror::Error;

type BlockIndex = u32;

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
    mount_point: String,
    block_size: u32,
    num_blocks: u32,
    blocks_in_group: u32,
}

struct free_table {
    table: [u8; BLOCKS_IN_GROUP as usize / 8],
}

#[derive(Error, Debug)]
enum FreeTableError {
    #[error("The bit index is out of bounds. Bit index provided: {0}, Max bit index: {1}")]
    OutOfBounds(u32, u32),
}

impl free_table {
    fn from_fs(block_file: &mut File, block_group: u32) -> Self {
        let block_index = match block_group {
            0 => 1,
            _ => BLOCKS_IN_GROUP * block_group,
        };
        let mut table = [0 as u8; BLOCKS_IN_GROUP as usize / 8];
        block_file
            .read_exact_at(&mut table, (block_index * BLOCK_SIZE).into())
            .unwrap();
        free_table { table }
    }

    // Creates a new free table, writes it to the disk, and returns it
    fn create_and_init(block_file: &mut File, block_group: u32) -> Self {
        let block_index = match block_group {
            0 => 1,
            _ => BLOCKS_IN_GROUP * block_group,
        };
        let mut table = [0 as u8; BLOCKS_IN_GROUP as usize / 8];
        block_file
            .read_exact_at(&mut table, (block_index * BLOCK_SIZE).into())
            .unwrap();
        free_table { table }
    }

    fn set_bit(
        &mut self,
        block_file: &mut File,
        bit_index: u32,
        fs: &Dsfs,
    ) -> Result<(), FreeTableError> {
        // TODO: Check this condition (maybe off by 1)
        if bit_index >= fs.blocks_in_group {
            return Err(FreeTableError::OutOfBounds(bit_index, fs.blocks_in_group));
        }
        todo!();
        Ok(())
    }
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
    let mount_point = matches.get_one::<String>("MOUNT_POINT").unwrap();
    let fs_filename = matches.get_one::<String>("DEVICE_FILE").unwrap();
    let mut options = vec![MountOption::RW, MountOption::FSName("dsfs".to_string())];
    if matches.get_flag("no-auto-unmount") {
        options.push(MountOption::AutoUnmount);
    }
    if matches.get_flag("allow-root") {
        options.push(MountOption::AllowRoot);
    }
    println!("Mounting {} on {}", fs_filename, mount_point);
    let hello_fs = Dsfs {
        block_file: File::open(fs_filename).unwrap(),
        mount_point: mount_point.to_string(),
        block_size: BLOCK_SIZE,
        num_blocks: NUM_BLOCKS,
        blocks_in_group: BLOCKS_IN_GROUP,
    };
    fuser::mount2(hello_fs, mount_point, &options).unwrap();
}
