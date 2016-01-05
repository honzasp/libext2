use std::{io, fs};
use std::io::{Read, Write, Seek};
use error::{Result};

pub trait Volume {
  fn read(&mut self, offset: u64, buffer: &mut [u8]) -> Result<()>;
  fn write(&mut self, offset: u64, buffer: &[u8]) -> Result<()>;
}

pub struct FileVolume(pub fs::File);

impl Volume for FileVolume {
  #[cfg(feature = "read_exact")]
  fn read(&mut self, offset: u64, buffer: &mut [u8]) -> Result<()> {
    try!(self.0.seek(io::SeekFrom::Start(offset)));
    try!(self.0.read_exact(buffer));
    Ok(())
  }

  #[cfg(not(feature = "read_exact"))]
  fn read(&mut self, offset: u64, buffer: &mut [u8]) -> Result<()> {
    try!(self.0.seek(io::SeekFrom::Start(offset)));
    let mut total_read = 0;
    while total_read < buffer.len() {
      total_read += try!(self.0.read(&mut buffer[total_read..]));
    }
    Ok(())
  }

  fn write(&mut self, offset: u64, buffer: &[u8]) -> Result<()> {
    try!(self.0.seek(io::SeekFrom::Start(offset)));
    try!(self.0.write_all(buffer));
    Ok(())
  }
}
