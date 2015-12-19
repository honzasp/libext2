use std::{io, fs};
use std::io::{Read, Seek};
use error::{Result};

pub trait ReadRaw {
  fn read(&mut self, offset: u64, buffer: &mut [u8]) -> Result<()>;
}

pub struct FileReader(pub fs::File);

impl ReadRaw for FileReader {
  fn read(&mut self, offset: u64, buffer: &mut [u8]) -> Result<()> {
    try!(self.0.seek(io::SeekFrom::Start(offset)));
    let mut pos = 0;
    while pos < buffer.len() {
      pos = pos + try!(self.0.read(&mut buffer[pos..]));
    }
    Ok(())
  }
}
