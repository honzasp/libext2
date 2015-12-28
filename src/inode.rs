use std::{cmp};
use prelude::*;

pub fn get_inode(fs: &mut Filesystem, ino: u64) -> Result<Inode> {
  match fs.inodes.get(&ino) {
    Some(inode) => return Ok(inode.clone()),
    None => (),
  }
  let inode = try!(read_inode(fs, ino));
  fs.inodes.insert(ino, inode.clone());
  Ok(inode)
}

pub fn inode_mode_from_linux_mode(mode: u16) -> Result<Mode> {
  decode_inode_mode(mode)
}

pub fn make_inode_in_dir(fs: &mut Filesystem, dir_ino: u64,
  name: &[u8], mode: Mode) -> Result<Inode>
{
  let mut dir_inode = try!(get_inode(fs, dir_ino));
  if dir_inode.mode.file_type != FileType::Dir {
    return Err(Error::new(format!(
      "Inode {} is not a directory", dir_ino)));
  }

  let dir_group = get_ino_group(fs, dir_ino).0;
  let new_ino = match try!(alloc_inode(fs, dir_group)) {
    None => return Err(Error::new(format!("No free inodes left"))),
    Some(ino) => ino,
  };

  let mut new_inode = try!(init_inode(fs, &mut dir_inode, new_ino, mode));
  try!(add_dir_entry(fs, &mut dir_inode, &mut new_inode, name));
  Ok(new_inode)
}

pub fn update_inode(fs: &mut Filesystem, inode: &Inode) -> Result<()> {
  fs.inodes.insert(inode.ino, inode.clone());
  fs.dirty_inos.insert(inode.ino);
  Ok(())
}

pub fn flush_ino(fs: &mut Filesystem, ino: u64) -> Result<()> {
  if let Some(inode) = fs.inodes.remove(&ino) {
    if fs.dirty_inos.remove(&ino) {
      return write_inode(fs, &inode);
    }
  }
  Ok(())
}

fn read_inode(fs: &mut Filesystem, ino: u64) -> Result<Inode> {
  let (offset, inode_size) = try!(locate_inode(fs, ino));
  let mut inode_buf = make_buffer(inode_size);
  try!(fs.volume.read(offset, &mut inode_buf[..]));
  decode_inode(&fs.superblock, ino, &inode_buf[..])
}

fn write_inode(fs: &mut Filesystem, inode: &Inode) -> Result<()> {
  println!("write_inode({:?})", inode);
  let (offset, inode_size) = try!(locate_inode(fs, inode.ino));
  let mut inode_buf = make_buffer(inode_size);
  try!(encode_inode(&fs.superblock, inode, &mut inode_buf[..]));
  fs.volume.write(offset, &inode_buf[..])
}

fn locate_inode(fs: &mut Filesystem, ino: u64) -> Result<(u64, u64)> {
  let (group_idx, local_idx) = get_ino_group(fs, ino);
  let inode_size = fs.superblock.inode_size as u64;
  let inode_table = fs.groups[group_idx as usize].desc.inode_table as u64;
  let offset = inode_table * fs.block_size() + local_idx * inode_size;
  Ok((offset, inode_size))
}

fn init_inode(fs: &mut Filesystem, dir_inode: &mut Inode,
  ino: u64, mode: Mode) -> Result<Inode> 
{
  let mut inode = Inode {
    ino: ino,
    mode: mode,
    uid: 0, gid: 0,
    size: 0, size_512: 0,
    atime: 0, ctime: 0, mtime: 0,
    links_count: 0, flags: 0,
    block: [0; 15],
    file_acl: 0,
  };

  if mode.file_type == FileType::Dir {
    try!(init_dir(fs, dir_inode, &mut inode));
  }
  try!(update_inode(fs, &inode));
  Ok(inode)
}

#[derive(Copy, Clone, Debug)]
enum BlockPos {
  Level0(u64),
  Level1(u64),
  Level2(u64, u64),
  Level3(u64, u64, u64),
  OutOfRange,
}

pub fn read_inode_data(fs: &mut Filesystem, inode: &Inode, 
  offset: u64, buffer: &mut [u8]) -> Result<u64> 
{
  let block_size = fs.block_size();
  let max_length = cmp::min(buffer.len() as u64, inode.size - offset);
  let mut chunk_begin = 0;
  while chunk_begin < max_length {
    let chunk_block = (offset + chunk_begin) / block_size;
    let chunk_offset = (offset + chunk_begin) % block_size;
    let chunk_length = cmp::min(max_length - chunk_begin,
        block_size - chunk_offset);
    try!(read_inode_block(fs, inode, chunk_block, chunk_offset,
          &mut (buffer[chunk_begin as usize..])[0..chunk_length as usize]));
    chunk_begin = chunk_begin + chunk_length;
  }
  Ok(chunk_begin)
}

pub fn read_inode_block(fs: &mut Filesystem, inode: &Inode, inode_block: u64,
  offset: u64, buffer: &mut [u8]) -> Result<()>
{
  assert!(offset + buffer.len() as u64 <= fs.block_size());
  let real_block = match try!(get_inode_block(fs, inode, inode_block)) {
    Some(block) => block,
    None => return Err(Error::new(
          format!("File block {} is out of the allocated range", inode_block))),
  };
  let block_offset = real_block * fs.block_size() + offset;
  fs.volume.read(block_offset, buffer)
}

fn read_indirect(fs: &mut Filesystem, indirect_block: u64, entry: u64) -> Result<u64> {
  let mut buffer = [0; 4];
  let entry_offset = indirect_block * fs.block_size() + entry * 4;
  assert!(entry < fs.block_size() / 4);
  try!(fs.volume.read(entry_offset, &mut buffer[..]));
  Ok(decode_u32(&buffer[..]) as u64)
}

pub fn write_inode_data(fs: &mut Filesystem, inode: &mut Inode,
  offset: u64, buffer: &[u8]) -> Result<u64>
{
  let block_size = fs.block_size();
  let mut chunk_begin = 0;
  while chunk_begin < buffer.len() as u64 {
    let chunk_block = (offset + chunk_begin) / block_size;
    let chunk_offset = (offset + chunk_begin) % block_size;
    let chunk_length = cmp::min(buffer.len() as u64 - chunk_begin,
        block_size - chunk_offset);
    try!(write_inode_block(fs, inode, chunk_block, chunk_offset,
          &(buffer[chunk_begin as usize..])[0..chunk_length as usize]));
    chunk_begin = chunk_begin + chunk_length;
  }

  if inode.size < offset + chunk_begin {
    inode.size = offset + chunk_begin;
    try!(update_inode(fs, inode));
  }

  Ok(chunk_begin)
}

fn write_inode_block(fs: &mut Filesystem, inode: &mut Inode, inode_block: u64,
  offset: u64, buffer: &[u8]) -> Result<()>
{
  assert!(offset + buffer.len() as u64 <= fs.block_size());
  let real_block = match try!(get_inode_block(fs, inode, inode_block)) {
    Some(block) => block,
    None => {
      let block = try!(alloc_inode_block(fs, inode));
      try!(set_inode_block(fs, inode, inode_block, block));
      block
    }
  };
  let block_offset = real_block * fs.block_size() + offset;
  fs.volume.write(block_offset, buffer)
}

fn write_indirect(fs: &mut Filesystem, indirect_block: u64,
  entry: u64, link: u64) -> Result<()> 
{
  let mut buffer = [0; 4];
  let entry_offset = indirect_block * fs.block_size() + entry * 4;
  assert!(entry < fs.block_size() / 4);
  encode_u32(link as u32, &mut buffer[..]);
  fs.volume.write(entry_offset, &buffer[..])
}


fn alloc_inode_block(fs: &mut Filesystem, inode: &mut Inode) -> Result<u64> {
  let (inode_group_idx, _) = get_ino_group(fs, inode.ino);
  match try!(alloc_block(fs, inode_group_idx)) {
    Some(block) => {
      inode.size_512 += (fs.block_size() / 512) as u32;
      try!(update_inode(fs, inode));
      Ok(block)
    },
    None => Err(Error::new(format!("No free blocks remain for files"))),
  }
}

fn alloc_indirect_block(fs: &mut Filesystem, inode: &mut Inode) -> Result<u64> {
  let (inode_group_idx, _) = get_ino_group(fs, inode.ino);
  let block = match try!(alloc_block(fs, inode_group_idx)) {
    Some(block) => block,
    None => return Err(Error::new(
        format!("No free blocks remain for indirections"))),
  };

  inode.size_512 += (fs.block_size() / 512) as u32;
  try!(update_inode(fs, inode));

  let zeros = make_buffer(fs.block_size());
  let offset = block * fs.block_size();
  try!(fs.volume.write(offset, &zeros[..]));
  Ok(block)
}

fn get_inode_block(fs: &mut Filesystem, inode: &Inode,
  inode_block: u64) -> Result<Option<u64>> 
{
  Ok(Some(match inode_block_to_pos(fs, inode_block) {
    BlockPos::Level0(level0) => {
      let block0 = inode.block[level0 as usize] as u64;
      if block0 == 0 { return Ok(None) } else { block0 }
    },
    BlockPos::Level1(level0) => {
      let block1 = inode.block[12] as u64;
      if block1 == 0 { return Ok(None) }
      let block0 = try!(read_indirect(fs, block1, level0));
      if block0 == 0 { return Ok(None) } else { block0 }
    },
    BlockPos::Level2(level1, level0) => {
      let block2 = inode.block[13] as u64;
      if block2 == 0 { return Ok(None) }
      let block1 = try!(read_indirect(fs, block2, level1));
      if block1 == 0 { return Ok(None) }
      let block0 = try!(read_indirect(fs, block1, level0));
      if block0 == 0 { return Ok(None) } else { block0 }
    },
    BlockPos::Level3(level2, level1, level0) => {
      let block3 = inode.block[14] as u64;
      if block3 == 0 { return Ok(None) }
      let block2 = try!(read_indirect(fs, block3, level2));
      if block2 == 0 { return Ok(None) }
      let block1 = try!(read_indirect(fs, block2, level1));
      if block1 == 0 { return Ok(None) }
      let block0 = try!(read_indirect(fs, block1, level0));
      if block0 == 0 { return Ok(None) } else { block0 }
    },
    BlockPos::OutOfRange =>
      return Err(Error::new(
        format!("File block {} is out of range for reading", inode_block))),
  }))
}

fn set_inode_block(fs: &mut Filesystem, inode: &mut Inode,
  inode_block: u64, block: u64) -> Result<()> 
{
  if let Some(prev_block) = try!(get_inode_block(fs, inode, inode_block)) {
    panic!("inode {}, file block {}: tried to overwrite block {} with {}",
            inode.ino, inode_block, prev_block, block);
  }

  let inode_indirect = |fs: &mut Filesystem, inode: &mut Inode,
    idx: u64| -> Result<_> 
  {
    if inode.block[idx as usize] == 0 {
      inode.block[idx as usize] = try!(alloc_indirect_block(fs, inode)) as u32;
      println!("allocate indirect block {} for direct block {} in inode",
               inode.block[idx as usize], idx);
      try!(update_inode(fs, inode));
    }
    Ok(inode.block[idx as usize] as u64)
  };

  let block_indirect = |fs: &mut Filesystem, inode: &mut Inode,
    indirect: u64, entry: u64| -> Result<_> 
  {
    let old_block = try!(read_indirect(fs, indirect, entry));
    if old_block == 0 {
      let new_block = try!(alloc_indirect_block(fs, inode));
      try!(write_indirect(fs, indirect, entry, new_block));
      println!("allocate indirect block {} for entry {} in indirect block {}",
               new_block, entry, indirect);
      Ok(new_block)
    } else {
      Ok(old_block)
    }
  };

  match inode_block_to_pos(fs, inode_block) {
    BlockPos::Level0(level0) => {
      inode.block[level0 as usize] = block as u32;
      try!(update_inode(fs, inode));
    },
    BlockPos::Level1(level0) => {
      let block1 = try!(inode_indirect(fs, inode, 12));
      try!(write_indirect(fs, block1, level0, block));
    },
    BlockPos::Level2(level1, level0) => {
      let block2 = try!(inode_indirect(fs, inode, 13));
      let block1 = try!(block_indirect(fs, inode, block2, level1));
      try!(write_indirect(fs, block1, level0, block));
    },
    BlockPos::Level3(level2, level1, level0) => {
      let block3 = try!(inode_indirect(fs, inode, 14));
      let block2 = try!(block_indirect(fs, inode, block3, level2));
      let block1 = try!(block_indirect(fs, inode, block2, level1));
      try!(write_indirect(fs, block1, level0, block));
    },
    BlockPos::OutOfRange =>
      return Err(Error::new(
          format!("File block {} is out of range for writing", inode_block))),
  }

  Ok(())
}

fn inode_block_to_pos(fs: &Filesystem, inode_block: u64) -> BlockPos {
  let indirect_1_size: u64 = fs.block_size() / 4;
  let indirect_2_size = indirect_1_size * indirect_1_size;
  let indirect_3_size = indirect_1_size * indirect_2_size;
  if inode_block < 12 {
    BlockPos::Level0(inode_block)
  } else if inode_block < 12 + indirect_1_size {
    BlockPos::Level1(inode_block - 12)
  } else if inode_block < 12 + indirect_1_size + indirect_2_size {
    BlockPos::Level2((inode_block - 12) / indirect_1_size,
      (inode_block - 12) % indirect_1_size)
  } else if inode_block < 12 + indirect_1_size + indirect_2_size + indirect_3_size {
    BlockPos::Level3((inode_block - 12) / indirect_2_size,
      ((inode_block - 12) % indirect_2_size) / indirect_1_size,
      ((inode_block - 12) % indirect_2_size) % indirect_1_size)
  } else {
    BlockPos::OutOfRange
  }
}

pub fn get_ino_group(fs: &Filesystem, ino: u64) -> (u64, u64) {
  let group_size = fs.superblock.inodes_per_group as u64;
  ((ino - 1) / group_size, (ino - 1) % group_size)
}