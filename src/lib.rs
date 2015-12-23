#![feature(associated_consts)]
pub use defs::*;
pub use error::{Error, Result};
pub use filesystem::{Filesystem, DirHandle, FileHandle};
pub use read_raw::{ReadRaw, FileReader};

mod decode;
mod defs;
mod error;
mod filesystem;
mod read_raw;
