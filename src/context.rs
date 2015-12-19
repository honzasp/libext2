use std::{cell, iter};
use error::{Result};
use read_raw::{ReadRaw};
use superblock::{Superblock};
use inode::{Inode};
use group_desc::{GroupDesc};

pub struct Context {
  reader: cell::RefCell<Box<ReadRaw>>,
  superblock: Superblock,
}

impl Context {
  pub fn new(mut reader: Box<ReadRaw>) -> Result<Context> {
    let mut superblock_buf = make_buffer(1024);
    try!(reader.read(1024, &mut superblock_buf[..]));
    let superblock = try!(Superblock::decode(&superblock_buf[..], true));
    Ok(Context {
      reader: cell::RefCell::new(reader),
      superblock: superblock,
    })
  }

  pub fn read_inode(&self, ino: u64) -> Result<Inode> {
    let group_size = self.superblock.inodes_per_group as u64;
    let inode_size = self.superblock.inode_size as u64;
    let (group_idx, local_idx) = ((ino - 1) / group_size, (ino - 1) % group_size);
    let group_desc = try!(self.read_group_desc(group_idx));
    let offset = group_desc.inode_table as u64 * self.block_size() 
        + local_idx * inode_size;

    let mut inode_buf = make_buffer(inode_size as usize);
    try!(self.read(offset, &mut inode_buf[..]));
    Inode::decode(&self.superblock, &inode_buf[..])
  }

  pub fn read_group_desc(&self, group_idx: u64) -> Result<GroupDesc> {
    let group_desc_block = self.superblock.first_data_block as u64 + 1;
    let offset = group_desc_block * self.block_size() + group_idx * 32;
    let mut desc_buf = make_buffer(32);
    try!(self.read(offset, &mut desc_buf[..]));
    GroupDesc::decode(&self.superblock, &desc_buf[..])
  }

  pub fn read(&self, offset: u64, buffer: &mut [u8]) -> Result<()> {
    try!(self.reader.borrow_mut().read(offset, buffer));
    Ok(())
  }

  pub fn superblock(&self) -> &Superblock {
    &self.superblock 
  }

  pub fn block_size(&self) -> u64 {
    1024 << self.superblock.log_block_size 
  }
}

fn make_buffer(size: usize) -> Vec<u8> {
  iter::repeat(0).take(size).collect()
}
