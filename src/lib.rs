#![feature(associated_consts)]
pub use defs::*;
pub use error::{Error, Result};
pub use volume::{Volume, FileVolume};
pub use fs::{Filesystem, mount_fs, flush_fs};
pub use inode::{get_inode, set_inode_mode_attr, truncate_inode_size,
  inode_mode_from_linux_mode };
pub use make_in_dir::{make_inode_in_dir, make_symlink_in_dir, make_hardlink_in_dir};
pub use dir::{DirHandle, lookup_in_dir, remove_from_dir, move_between_dirs,
  open_dir, read_dir, close_dir};
pub use file::{FileHandle, open_file, read_file, write_file, close_file};
pub use link::{read_link};

mod alloc;
mod decode;
mod defs;
mod dir;
mod encode;
mod error;
mod file;
mod fs;
mod group;
mod inode;
mod inode_data;
mod make_in_dir;
mod link;
mod prelude;
mod volume;
