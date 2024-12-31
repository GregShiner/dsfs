mod dsfs;
// mod example_impl;
mod fs_structs;

use crate::dsfs::Dsfs;

use clap::{crate_version, value_parser, Arg, ArgAction, Command};
use dsfs::DsfsError;
use fuser::MountOption;
use std::path::PathBuf;

type BlockIndex = u32;
type GroupIndex = u32;

fn main() -> Result<(), DsfsError> {
    let matches = Command::new("dsfs")
        .version(crate_version!())
        .author("Gregory Shiner")
        .arg(
            Arg::new("MOUNT_POINT")
                .required(false)
                .index(1)
                .help("Act as a client, and mount FUSE at given path")
                .default_value("./mnt")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("DEVICE_FILE")
                .required(false)
                .index(2)
                .help("Mount a dsfs filesystem stored in a specific block device file")
                .default_value("dsfs.img")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("create-fs")
                .short('c')
                .long("create-fs")
                .value_name("block_size")
                .value_parser(value_parser!(u32).range(1..))
                .num_args(0..=1)
                .require_equals(false)
                .default_missing_value("4")
                .help(
                    "initializes a new filesystem at the given device file. Block size is in KiB",
                ),
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
    println!(
        "Mounting {:?} on {:?}",
        fs_filename.clone(),
        mount_point.clone()
    );
    let dsfs = match matches.get_one::<u32>("create-fs") {
        Some(block_size) => {
            Dsfs::create_and_write(fs_filename.clone(), mount_point.clone(), *block_size * 1024)
                .unwrap()
        }
        None => Dsfs::load(fs_filename.clone(), mount_point.clone())?,
    };
    println!("{:?}", dsfs);
    fuser::mount2(dsfs, mount_point, &options)?;
    Ok(())
}
