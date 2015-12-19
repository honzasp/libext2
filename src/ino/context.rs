use std::{cell, iter};
use error::{Result};
use ino::{Inode, DataReader, Superblock};
use block_read::{BlockRead};

pub struct Context {
  reader: cell::RefCell<Box<BlockRead>>,
  pub superblock: Superblock,
}

impl Context {
  pub fn new(mut reader: Box<BlockRead>) -> Result<Context> {
    let mut superblock_bytes: Vec<u8> = iter::repeat(0).take(1024).collect();
    try!(reader.read(1024, &mut superblock_bytes[..]));
    let superblock = try!(Superblock::decode(&superblock_bytes[..], true));
    Ok(Context {
      reader: cell::RefCell::new(reader),
      superblock: superblock,
    })
  }

  pub fn read_inode(&self, _ino: u64) -> Result<Inode> {
    panic!("Reading inodes not implemented")
  }

  pub fn data_reader<'c>(&'c self, inode: &Inode) -> DataReader<'c> {
    DataReader::new(self, inode)
  }

  pub fn block_size(&self) -> u64 {
    1024 << self.superblock.log_block_size 
  }

  pub fn read(&self, offset: u64, buffer: &mut [u8]) -> Result<()> {
    try!(self.reader.borrow_mut().read(offset, buffer));
    Ok(())
  }
}

