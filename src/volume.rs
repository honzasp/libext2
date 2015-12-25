use std::{io, fs};
use std::io::{Read, Write, Seek};
use error::{Result};

pub trait Volume {
  fn read(&mut self, offset: u64, buffer: &mut [u8]) -> Result<()>;
  fn write(&mut self, offset: u64, buffer: &[u8]) -> Result<()>;
}

pub struct FileVolume(pub fs::File);

impl Volume for FileVolume {
  fn read(&mut self, offset: u64, buffer: &mut [u8]) -> Result<()> {
    try!(self.0.seek(io::SeekFrom::Start(offset)));
    try!(self.0.read_exact(buffer));
    Ok(())
  }

  fn write(&mut self, offset: u64, buffer: &[u8]) -> Result<()> {
    try!(self.0.seek(io::SeekFrom::Start(offset)));
    try!(self.0.write_all(buffer));
    Ok(())
  }
}
