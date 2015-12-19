use std::{io, fs};
use std::io::{Read, Seek};

pub trait BlockRead {
  fn read(&mut self, offset: u64, buffer: &mut [u8]) -> io::Result<()>;
}

pub struct FileRead {
  file: fs::File,
}

impl FileRead {
  pub fn new(file: fs::File) -> FileRead {
    FileRead { file: file }
  }
}

impl BlockRead for FileRead {
  fn read(&mut self, offset: u64, buffer: &mut [u8]) -> io::Result<()> {
    try!(self.file.seek(io::SeekFrom::Start(offset)));
    let mut pos = 0;
    while pos < buffer.len() {
      pos = pos + try!(self.file.read(&mut buffer[pos..]));
    }
    Ok(())
  }
}
