use prelude::*;

pub fn encode_superblock(superblock: &Superblock, bytes: &mut [u8]) -> Result<()> {
  encode_u32(superblock.blocks_count, &mut bytes[4..]);
  encode_u32(superblock.free_blocks_count, &mut bytes[12..]);
  encode_u32(superblock.free_inodes_count, &mut bytes[16..]);
  encode_u32(superblock.first_data_block, &mut bytes[20..]);
  encode_u32(superblock.log_block_size, &mut bytes[24..]);
  encode_u32(superblock.blocks_per_group, &mut bytes[32..]);
  encode_u32(superblock.inodes_per_group, &mut bytes[40..]);
  encode_u16(Superblock::MAGIC, &mut bytes[56..]);
  encode_u16(superblock.state, &mut bytes[58..]);
  encode_u32(superblock.rev_level, &mut bytes[76..]);

  if superblock.rev_level >= 1 {
    encode_u32(superblock.first_ino, &mut bytes[84..]);
    encode_u16(superblock.inode_size, &mut bytes[88..]);
    encode_u32(superblock.feature_compat, &mut bytes[92..]);
    encode_u32(superblock.feature_incompat, &mut bytes[96..]);
    encode_u32(superblock.feature_ro_compat, &mut bytes[100..]);
  }

  Ok(())
}

pub fn encode_group_desc(_superblock: &Superblock,
  group_desc: &GroupDesc, bytes: &mut [u8]) -> Result<()>
{
  encode_u32(group_desc.block_bitmap, &mut bytes[0..]);
  encode_u32(group_desc.inode_bitmap, &mut bytes[4..]);
  encode_u32(group_desc.inode_table, &mut bytes[8..]);
  encode_u16(group_desc.free_blocks_count, &mut bytes[12..]);
  encode_u16(group_desc.free_inodes_count, &mut bytes[14..]);
  encode_u16(group_desc.used_dirs_count, &mut bytes[16..]);
  Ok(())
}

pub fn encode_inode(superblock: &Superblock, inode: &Inode,
  bytes: &mut [u8]) -> Result<()>
{
  assert!(bytes.len() >= 128);
  encode_u16(encode_inode_mode(&inode.mode), &mut bytes[0..]);

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
  encode_u32(inode.dtime, &mut bytes[20..]);
  encode_u16(inode.links_count, &mut bytes[26..]);
  encode_u32(inode.size_512, &mut bytes[28..]);
  encode_u32(inode.flags, &mut bytes[32..]);
  encode_u32(inode.file_acl, &mut bytes[104..]);
  Ok(())
}

fn encode_inode_mode(mode: &Mode) -> u16 {
  encode_mode_file_type(mode.file_type) +
    if mode.suid { 0x0800 } else { 0 } +
    if mode.sgid { 0x0400 } else { 0 } +
    if mode.sticky { 0x0200 } else { 0 } +
    mode.access_rights
}

fn encode_mode_file_type(file_type: FileType) -> u16 {
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

pub fn encode_dir_entry(_superblock: &Superblock, entry: &DirEntry,
  bytes: &mut [u8]) -> Result<()>
{
  encode_u32(entry.ino, &mut bytes[0..]);
  encode_u16(entry.rec_len, &mut bytes[4..]);
  bytes[6] = entry.name_len;
  bytes[7] = encode_dir_entry_file_type(entry.file_type);
  Ok(())
}

fn encode_dir_entry_file_type(file_type: Option<FileType>) -> u8 {
  match file_type {
    None => 0,
    Some(FileType::Regular) => 1,
    Some(FileType::Dir) => 2,
    Some(FileType::CharDev) => 3,
    Some(FileType::BlockDev) => 4,
    Some(FileType::Fifo) => 5,
    Some(FileType::Socket) => 6,
    Some(FileType::Symlink) => 7,
  }
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
