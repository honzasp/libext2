use error::{Result};
use read_int::{read_u32};
use superblock::{Superblock};

#[derive(Debug)]
pub struct GroupDesc {
  pub block_bitmap: u32,
  pub inode_bitmap: u32,
  pub inode_table: u32,
}

impl GroupDesc {
  pub fn decode(_superblock: &Superblock, bytes: &[u8]) -> Result<GroupDesc> {
    assert!(bytes.len() >= 32);
    Ok(GroupDesc {
      block_bitmap: read_u32(&bytes[0..]),
      inode_bitmap: read_u32(&bytes[4..]),
      inode_table: read_u32(&bytes[8..]),
    })
  }
}
