mod dsfs;
mod example_impl;
mod fs_structs;

use crate::dsfs::Dsfs;

use clap::{crate_version, Arg, ArgAction, Command};
use fuser::MountOption;
use std::path::PathBuf;

type BlockIndex = u32;
type GroupIndex = u32;

const BLOCK_SIZE: u32 = 4096; // 4KiB
const NUM_BLOCKS: u32 = 1024; // 1024 Blocks = 4.0MiB ~= 4.2MB

// const BLOCKS_IN_GROUP: u32 = BLOCK_SIZE * 8; // Number of blocks in a group. This is limited by the
//                                              // number of bits in a free table, which is a single full block

// Reworked block tables (previously free tables) to use a u8 for each block
const BLOCKS_IN_GROUP: u32 = BLOCK_SIZE;

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
    let dsfs = Dsfs::new(fs_filename.clone(), mount_point.clone()).unwrap();
    fuser::mount2(dsfs, mount_point, &options).unwrap();
}
