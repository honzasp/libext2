use std::{iter};
use prelude::*;

pub struct Filesystem {
  pub volume: Box<Volume>,
  pub superblock: Superblock,
}

impl Filesystem {
  pub const ROOT_INO: u64 = 2;

  pub fn block_size(&self) -> u64 {
    1024 << self.superblock.log_block_size 
  }

  pub fn group_count(&self) -> u64 {
    self.superblock.blocks_count as u64 / self.superblock.blocks_per_group as u64
  }
}

pub fn mount_fs(mut volume: Box<Volume>) -> Result<Filesystem> {
  let mut superblock_buf = make_buffer(1024);
  try!(volume.read(1024, &mut superblock_buf[..]));
  let superblock = try!(decode_superblock(&superblock_buf[..], true));
  Ok(Filesystem { volume: volume, superblock: superblock })
}

pub fn read_group_desc(fs: &mut Filesystem, group_idx: u64) -> Result<GroupDesc> {
  let group_desc_block = fs.superblock.first_data_block as u64 + 1;
  let offset = group_desc_block * fs.block_size() + group_idx * 32;
  let mut desc_buf = make_buffer(32);
  try!(fs.volume.read(offset, &mut desc_buf[..]));
  decode_group_desc(&fs.superblock, &desc_buf[..])
}

pub fn make_buffer(size: u64) -> Vec<u8> {
  iter::repeat(0).take(size as usize).collect()
}
