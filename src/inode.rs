use error::{Error, Result};
use read_int::{read_u16, read_u32};
use superblock::{Superblock};

#[derive(Debug)]
pub struct Inode {
  pub file_type: FileType,
  pub suid: bool,
  pub sgid: bool,
  pub access_rights: AccessRights,
  pub uid: u32,
  pub gid: u32,
  pub size: u64,
  pub flags: u32,
  pub block: [u32; 15],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FileType {
  Regular,
  Dir,
  CharDev,
  BlockDev,
  Fifo,
  Socket,
  Symlink,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct AccessRights(u16);

impl Inode {
  pub fn decode(superblock: &Superblock, bytes: &[u8]) -> Result<Inode> {
    assert!(bytes.len() >= 128);
    let mode = read_u16(&bytes[0..]);
    let file_type = try!(Inode::decode_file_type(mode));

    let size_low = read_u32(&bytes[4..]) as u64;
    let size_high =
      if superblock.rev_level >= 1 && file_type == FileType::Regular {
        read_u32(&bytes[108..])
      } else {
        0
      } as u64;

    let uid_low = read_u16(&bytes[2..]) as u32;
    let uid_high = read_u16(&bytes[120..]) as u32;
    let gid_low = read_u16(&bytes[24..]) as u32;
    let gid_high = read_u16(&bytes[122..]) as u32;

    let mut block = [0; 15];
    for i in 0..15 {
      block[i] = read_u32(&bytes[40 + 4*i ..])
    }

    Ok(Inode {
      file_type: file_type,
      suid: (mode & 0x0800) != 0,
      sgid: (mode & 0x0400) != 0,
      access_rights: AccessRights(mode & 0x01ff),
      size: size_low + (size_high << 32),
      uid: uid_low + (uid_high << 16),
      gid: gid_low + (gid_high << 16),
      flags: read_u32(&bytes[32..]),
      block: block,
    })
  }

  fn decode_file_type(mode: u16) -> Result<FileType> {
    let type_nibble = (mode & 0xf000) >> 12;
    Ok(match type_nibble {
      1  => FileType::Fifo,
      2  => FileType::CharDev,
      4  => FileType::Dir,
      6  => FileType::BlockDev,
      8  => FileType::Regular,
      10 => FileType::Symlink,
      12 => FileType::Socket,
      _ => return Err(Error::new(
          format!("Unknown file type 0x{:x}", type_nibble))),
    })
  }
}
