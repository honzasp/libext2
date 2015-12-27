use defs::*;
use error::{Error, Result};

pub fn decode_superblock(bytes: &[u8], read_only: bool) -> Result<Superblock> {
  assert!(bytes.len() >= 1024);
  let magic = decode_u16(&bytes[56..]);
  let state = decode_u16(&bytes[58..]);
  let rev = decode_u32(&bytes[76..]);
  let feature_compat = if rev >= 1 { decode_u32(&bytes[92..]) } else { 0 };
  let feature_incompat = if rev >= 1 { decode_u32(&bytes[96..]) } else { 0 };
  let feature_ro_compat = if rev >= 1 { decode_u32(&bytes[100..]) } else { 0 };

  if magic != Superblock::MAGIC {
    return Err(Error::new(
        format!("Bad magic 0x{:x}, expected 0x{:x}", magic, Superblock::MAGIC)));
  }

  if state != 1 {
    return Err(Error::new(format!("Volume is in an invalid state (0x{:x})", state)));
  }

  if (feature_incompat & !Superblock::SUPPORTED_INCOMPAT) != 0 {
    return Err(Error::new(format!("Volume uses incompatible features (0x{:x})",
            feature_incompat)));
  }

  if !read_only && (feature_ro_compat & !Superblock::SUPPORTED_RO_COMPAT) != 0 {
    return Err(Error::new(format!(
            "Volume uses incompatible features, only reading is possible (0x{:x})",
            feature_ro_compat)));
  }

  Ok(Superblock {
    blocks_count: decode_u32(&bytes[4..]),
    free_blocks_count: decode_u32(&bytes[12..]),
    free_inodes_count: decode_u32(&bytes[16..]),
    first_data_block: decode_u32(&bytes[20..]),
    log_block_size: decode_u32(&bytes[24..]),
    blocks_per_group: decode_u32(&bytes[32..]),
    inodes_per_group: decode_u32(&bytes[40..]),
    state: state,
    rev_level: rev,
    first_ino: if rev >= 1 { decode_u32(&bytes[84..]) } else { 11 },
    inode_size: if rev >= 1 { decode_u16(&bytes[88..]) } else { 128 },
    feature_compat: feature_compat,
    feature_incompat: feature_incompat,
    feature_ro_compat: feature_ro_compat,
  })
}

pub fn decode_group_desc(_superblock: &Superblock, bytes: &[u8]) -> Result<GroupDesc> {
  assert!(bytes.len() >= 32);
  Ok(GroupDesc {
    block_bitmap: decode_u32(&bytes[0..]),
    inode_bitmap: decode_u32(&bytes[4..]),
    inode_table: decode_u32(&bytes[8..]),
    free_blocks_count: decode_u16(&bytes[12..]),
    free_inodes_count: decode_u16(&bytes[14..]),
    used_dirs_count: decode_u16(&bytes[16..]),
  })
}

pub fn decode_inode(superblock: &Superblock, ino: u64, bytes: &[u8]) -> Result<Inode> {
  assert!(bytes.len() >= 128);
  let mode = decode_u16(&bytes[0..]);
  let file_type = try!(decode_inode_file_type(mode));

  let size_low = decode_u32(&bytes[4..]) as u64;
  let size_high =
    if superblock.rev_level >= 1 && file_type == FileType::Regular {
      decode_u32(&bytes[108..])
    } else {
      0
    } as u64;

  let uid_low = decode_u16(&bytes[2..]) as u32;
  let uid_high = decode_u16(&bytes[120..]) as u32;
  let gid_low = decode_u16(&bytes[24..]) as u32;
  let gid_high = decode_u16(&bytes[122..]) as u32;

  let mut block = [0; 15];
  for i in 0..15 {
    block[i] = decode_u32(&bytes[40 + 4*i..])
  }

  Ok(Inode {
    ino: ino,
    file_type: file_type,
    suid: (mode & 0x0800) != 0,
    sgid: (mode & 0x0400) != 0,
    sticky: (mode & 0x0200) != 0,
    access_rights: AccessRights(mode & 0x01ff),
    uid: uid_low + (uid_high << 16),
    gid: gid_low + (gid_high << 16),
    size: size_low + (size_high << 32),
    size_512: decode_u32(&bytes[28..]),
    atime: decode_u32(&bytes[8..]),
    ctime: decode_u32(&bytes[12..]),
    mtime: decode_u32(&bytes[16..]),
    links_count: decode_u16(&bytes[26..]),
    flags: decode_u32(&bytes[32..]),
    block: block,
    file_acl: decode_u32(&bytes[104..]),
  })
}

fn decode_inode_file_type(mode: u16) -> Result<FileType> {
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
        format!("Unknown file type {}", type_nibble))),
  })
}

pub fn decode_dir_entry(superblock: &Superblock, bytes: &[u8]) -> Result<DirEntry> {
  let file_type = if superblock.rev_level >= 1 {
      try!(decode_dir_entry_file_type(bytes[7]))
    } else {
      None
    };

  Ok(DirEntry {
    ino: decode_u32(&bytes[0..]),
    rec_len: decode_u16(&bytes[4..]),
    name_len: bytes[6],
    file_type: file_type,
  })
}

fn decode_dir_entry_file_type(byte: u8) -> Result<Option<FileType>> {
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

pub fn decode_u16(bytes: &[u8]) -> u16 {
  (bytes[0] as u16) +
  ((bytes[1] as u16) << 8)
}

pub fn decode_u32(bytes: &[u8]) -> u32 {
  (bytes[0] as u32) +
  ((bytes[1] as u32) << 8) +
  ((bytes[2] as u32) << 16) +
  ((bytes[3] as u32) << 24)
}
