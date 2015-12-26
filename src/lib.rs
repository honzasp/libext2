#![feature(associated_consts)]
pub use defs::*;
pub use error::{Error, Result};
pub use filesystem::{Filesystem};
pub use volume::{Volume, FileVolume};

pub mod fs {
  pub use filesystem::new as mount;
}

pub mod inode {
  pub use filesystem::read_inode as read;
}

pub mod dir {
  pub use filesystem::DirHandle as Handle;
  pub use filesystem::dir_lookup as lookup;
  pub use filesystem::dir_open as open;
  pub use filesystem::dir_read as read;
  pub use filesystem::dir_close as close;
}

pub mod file {
  pub use filesystem::FileHandle as Handle;
  pub use filesystem::file_open as open;
  pub use filesystem::file_read as read;
  pub use filesystem::file_write as write;
  pub use filesystem::file_close as close;
}

pub mod link {
  pub use filesystem::link_read as read;
}

mod decode;
mod encode;
mod defs;
mod error;
mod filesystem;
mod volume;
