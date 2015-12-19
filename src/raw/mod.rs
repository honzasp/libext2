use std::{io};

pub trait BlockRead {
  fn read(&self, offset: u64, buffer: &mut [u8]) -> io::Result<()>;
}
