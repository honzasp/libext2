use defs::*;
use error::{Error, Result};

pub fn encode_inode(superblock: &Superblock, inode: &Inode,
  bytes: &mut [u8]) -> Result<()>
{
  assert!(bytes.len() >= 128);
  encode_u16(encode_inode_mode(inode), &mut bytes[0..]);

  encode_u16((inode.uid & 0xffff) as u16, &mut bytes[2..]);
  encode_u16(((inode.uid >> 16) & 0xffff) as u16, &mut bytes[120..]);
  encode_u16((inode.gid & 0xffff) as u16, &mut bytes[24..]);
  encode_u16(((inode.gid >> 16) & 0xffff) as u16, &mut bytes[122..]);

  encode_u32((inode.size & 0xffffffff) as u32, &mut bytes[4..]);
  if (inode.size >> 32) != 0 && superblock.rev_level < 1 {
    return Err(Error::new(
      format!("Cannot encode file size exceeding 32 bits in rev {}",
              superblock.rev_level)));
  } else {
    encode_u32(((inode.size >> 32) & 0xffffffff) as u32, &mut bytes[108..]);
  }

  for i in 0..15 {
    encode_u32(inode.block[i], &mut bytes[40 + 4*i..]);
  }

  encode_u32(inode.atime, &mut bytes[8..]);
  encode_u32(inode.ctime, &mut bytes[12..]);
  encode_u32(inode.mtime, &mut bytes[16..]);
  encode_u16(inode.links_count, &mut bytes[26..]);
  encode_u32(inode.size_512, &mut bytes[28..]);
  encode_u32(inode.flags, &mut bytes[32..]);
  encode_u32(inode.file_acl, &mut bytes[104..]);
  Ok(())
}

fn encode_inode_mode(inode: &Inode) -> u16 {
  encode_inode_file_type(inode.file_type) +
    if inode.suid { 0x0800 } else { 0 } +
    if inode.sgid { 0x0400 } else { 0 } +
    if inode.sticky { 0x0200 } else { 0 } +
    inode.access_rights.0
}

fn encode_inode_file_type(file_type: FileType) -> u16 {
  (match file_type {
    FileType::Fifo => 1,
    FileType::CharDev => 2,
    FileType::Dir => 4,
    FileType::BlockDev => 6,
    FileType::Regular => 8,
    FileType::Symlink => 10,
    FileType::Socket => 12,
  }) << 12
}

pub fn encode_u32(value: u32, bytes: &mut [u8]) {
  bytes[0] = (value & 0xff) as u8;
  bytes[1] = ((value >> 8) & 0xff) as u8;
  bytes[2] = ((value >> 16) & 0xff) as u8;
  bytes[3] = ((value >> 24) & 0xff) as u8;
}

pub fn encode_u16(value: u16, bytes: &mut [u8]) {
  bytes[0] = (value & 0xff) as u8;
  bytes[1] = ((value >> 8) & 0xff) as u8;
}
