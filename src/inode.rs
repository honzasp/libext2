use std::{cmp};
use prelude::*;

pub fn read_inode(fs: &mut Filesystem, ino: u64) -> Result<Inode> {
  let (offset, inode_size) = try!(locate_inode(fs, ino));
  let mut inode_buf = make_buffer(inode_size);
  try!(fs.volume.read(offset, &mut inode_buf[..]));
  decode_inode(&fs.superblock, ino, &inode_buf[..])
}

pub fn write_inode(fs: &mut Filesystem, inode: &Inode) -> Result<()> {
  println!("write_inode {:?}", inode);
  let (offset, inode_size) = try!(locate_inode(fs, inode.ino));
  let mut inode_buf = make_buffer(inode_size);
  try!(encode_inode(&fs.superblock, inode, &mut inode_buf[..]));
  fs.volume.write(offset, &inode_buf[..])
}

fn locate_inode(fs: &mut Filesystem, ino: u64) -> Result<(u64, u64)> {
  let (group_idx, local_idx) = get_ino_group(fs, ino);
  let inode_size = fs.superblock.inode_size as u64;
  let group_desc = try!(read_group_desc(fs, group_idx));
  let offset = group_desc.inode_table as u64 * fs.block_size() 
      + local_idx * inode_size;
  Ok((offset, inode_size))
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
  println!(".write_inode_data(ino {}, offset {}, len {})",
    inode.ino, offset, buffer.len());
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
    try!(write_inode(fs, inode));
  }

  Ok(chunk_begin)
}

fn write_inode_block(fs: &mut Filesystem, inode: &mut Inode, inode_block: u64,
  offset: u64, buffer: &[u8]) -> Result<()>
{
  println!(".write_inode_block(ino {}, inode_block {}, offset {}, len {})",
    inode.ino, inode_block, offset, buffer.len());
  assert!(offset + buffer.len() as u64 <= fs.block_size());
  let real_block = match try!(get_inode_block(fs, inode, inode_block)) {
    Some(block) => block,
    None => {
      let block = try!(alloc_inode_block(fs, inode));
      try!(set_inode_block(fs, inode, inode_block, block));
      block
    }
  };
  println!("write ino {}, inode_block {} => block {}",
            inode.ino, inode_block, real_block);
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
  println!(".alloc_inode_block(ino {})", inode.ino);
  let (inode_group_idx, _) = get_ino_group(fs, inode.ino);
  match try!(alloc_block(fs, inode_group_idx)) {
    Some(block) => Ok(block),
    None => Err(Error::new(format!("No free blocks remain for files"))),
  }
}

fn alloc_indirect_block(fs: &mut Filesystem, inode: &mut Inode) -> Result<u64> {
  println!(".alloc_indirect_block(ino {})", inode.ino);
  let (inode_group_idx, _) = get_ino_group(fs, inode.ino);
  let block = match try!(alloc_block(fs, inode_group_idx)) {
    Some(block) => block,
    None => return Err(Error::new(
        format!("No free blocks remain for indirections"))),
  };

  let zeros = make_buffer(fs.block_size());
  let offset = block * fs.block_size();
  try!(fs.volume.write(offset, &zeros[..]));
  Ok(block)
}

fn get_inode_block(fs: &mut Filesystem, inode: &Inode, inode_block: u64) -> Result<Option<u64>> {
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
  println!(".set_inode_block(ino {}, inode_block {}, block {})",
    inode.ino, inode_block, block);

  if let Some(prev_block) = try!(get_inode_block(fs, inode, inode_block)) {
    panic!("inode {}, file block {}: tried to overwrite block {} with {}",
            inode.ino, inode_block, prev_block, block);
  }

  let inode_indirect = |fs: &mut Filesystem, inode: &mut Inode,
    idx: u64| -> Result<_> 
  {
    if inode.block[idx as usize] == 0 {
      inode.block[idx as usize] = try!(alloc_indirect_block(fs, inode)) as u32;
      try!(write_inode(fs, inode));
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
      Ok(new_block)
    } else {
      Ok(old_block)
    }
  };

  match inode_block_to_pos(fs, inode_block) {
    BlockPos::Level0(level0) => {
      inode.block[level0 as usize] = block as u32;
      try!(write_inode(fs, inode));
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

  let min_size_512 = ((inode_block + 1) * fs.block_size() / 512) as u32;
  if inode.size_512 < min_size_512 {
    inode.size_512 = min_size_512;
    write_inode(fs, inode)
  } else {
    Ok(())
  }
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

fn get_ino_group(fs: &Filesystem, ino: u64) -> (u64, u64) {
  let group_size = fs.superblock.inodes_per_group as u64;
  ((ino - 1) / group_size, (ino - 1) % group_size)
}
