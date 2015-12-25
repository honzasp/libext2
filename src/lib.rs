#![feature(associated_consts)]
pub use defs::*;
pub use error::{Error, Result};
pub use filesystem::{Filesystem, DirHandle, FileHandle};
pub use volume::{Volume, FileVolume};

mod decode;
mod encode;
mod defs;
mod error;
mod filesystem;
mod volume;
