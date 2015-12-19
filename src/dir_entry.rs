use superblock::{Superblock};
use inode::{FileType};
use error::{Result, Error};
use read_int::{read_u16, read_u32};

#[derive(Debug)]
pub struct DirEntry {
  pub ino: u32,
  pub rec_len: u16,
  pub file_type: Option<FileType>,
  pub name: Vec<u8>,
}

impl DirEntry {
  pub fn decode(superblock: &Superblock, bytes: &[u8]) -> Result<DirEntry> {
    let name_len = bytes[6] as usize;
    let file_type = if superblock.rev_level >= 1 {
        try!(DirEntry::decode_file_type(bytes[7]))
      } else {
        None
      };

    Ok(DirEntry {
      ino: read_u32(&bytes[0..]),
      rec_len: read_u16(&bytes[4..]),
      file_type: file_type,
      name: bytes[8..8+name_len].to_vec(),
    })
  }

  fn decode_file_type(byte: u8) -> Result<Option<FileType>> {
    if byte == 0 {
      return Ok(None)
    }
    Ok(Some(match byte {
      1 => FileType::Regular,
      2 => FileType::Dir,
      3 => FileType::CharDev,
      4 => FileType::BlockDev,
      5 => FileType::Fifo,
      6 => FileType::Socket,
      7 => FileType::Symlink,
      _ => return Err(Error::new(format!("Unknown file type {}", byte))),
    }))
  }
}
