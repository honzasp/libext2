use std::{cmp, iter};
use defs::*;
use decode;
use encode;
use error::{Error, Result};
use volume::{Volume};

pub struct Filesystem {
  volume: Box<Volume>,
  superblock: Superblock,
}

#[derive(Debug)]
pub struct DirHandle {
  inode: Inode,
  offset: u64,
  cache: Option<(u64, Vec<u8>)>,
}

#[derive(Debug)]
pub struct DirLine {
  pub ino: u64,
  pub file_type: FileType,
  pub name: Vec<u8>,
}

#[derive(Debug)]
pub struct FileHandle {
  inode: Inode,
}

#[derive(Copy, Clone, Debug)]
enum BlockPos {
  Level0(u64),
  Level1(u64),
  Level2(u64, u64),
  Level3(u64, u64, u64),
  OutOfRange,
}


impl Filesystem {
  pub const ROOT_INO: u64 = 2;

  pub fn new(mut volume: Box<Volume>) -> Result<Filesystem> {
    let mut superblock_buf = make_buffer(1024);
    try!(volume.read(1024, &mut superblock_buf[..]));
    let superblock = try!(decode::decode_superblock(&superblock_buf[..], true));
    Ok(Filesystem { volume: volume, superblock: superblock })
  }

  pub fn read_inode(&mut self, ino: u64) -> Result<Inode> {
    let (offset, inode_size) = try!(self.locate_inode(ino));
    let mut inode_buf = make_buffer(inode_size);
    try!(self.volume.read(offset, &mut inode_buf[..]));
    decode::decode_inode(&self.superblock, ino, &inode_buf[..])
  }

  fn write_inode(&mut self, inode: &Inode) -> Result<()> {
    println!("write_inode {:?}", inode);
    let (offset, inode_size) = try!(self.locate_inode(inode.ino));
    let mut inode_buf = make_buffer(inode_size);
    try!(encode::encode_inode(&self.superblock, inode, &mut inode_buf[..]));
    self.volume.write(offset, &inode_buf[..])
  }

  fn locate_inode(&mut self, ino: u64) -> Result<(u64, u64)> {
    let (group_idx, local_idx) = self.get_ino_group(ino);
    let inode_size = self.superblock.inode_size as u64;
    let group_desc = try!(self.read_group_desc(group_idx));
    let offset = group_desc.inode_table as u64 * self.block_size() 
        + local_idx * inode_size;
    Ok((offset, inode_size))
  }

  fn read_group_desc(&mut self, group_idx: u64) -> Result<GroupDesc> {
    let group_desc_block = self.superblock.first_data_block as u64 + 1;
    let offset = group_desc_block * self.block_size() + group_idx * 32;
    let mut desc_buf = make_buffer(32);
    try!(self.volume.read(offset, &mut desc_buf[..]));
    decode::decode_group_desc(&self.superblock, &desc_buf[..])
  }

  fn block_size(&self) -> u64 {
    1024 << self.superblock.log_block_size 
  }

  fn group_count(&self) -> u64 {
    self.superblock.blocks_count as u64 / self.superblock.blocks_per_group as u64
  }

  pub fn dir_lookup(&mut self, dir_inode: Inode, name: &[u8]) -> Result<Option<u64>> {
    let mut handle = try!(self.dir_open(dir_inode));
    while let Some(line) = try!(self.dir_read(&mut handle)) {
      if line.name == name {
        return Ok(Some(line.ino));
      }
    }
    Ok(None)
  }

  pub fn dir_open(&mut self, inode: Inode) -> Result<DirHandle> {
    if inode.file_type == FileType::Dir {
      Ok(DirHandle { inode: inode, offset: 0, cache: None })
    } else {
      return Err(Error::new(format!("inode is not a directory")))
    }
  }

  pub fn dir_read(&mut self, handle: &mut DirHandle) -> Result<Option<DirLine>> {
    if handle.offset >= handle.inode.size {
      return Ok(None)
    }
    let block_idx = handle.offset / self.block_size();
    let block_pos = handle.offset % self.block_size();

    let cache_valid = if let Some((cached_idx, _)) = handle.cache {
        cached_idx == block_idx
      } else {
        false
      };

    if !cache_valid {
      let mut buffer = make_buffer(self.block_size());
      try!(self.read_file_block(&handle.inode, block_idx, 0, &mut buffer[..]));
      handle.cache = Some((block_idx, buffer));
    }

    let block = &handle.cache.as_ref().unwrap().1;
    let entry = try!(decode::decode_dir_entry(
        &self.superblock, &block[block_pos as usize..]));
    let file_type = match entry.file_type {
      Some(file_type) => file_type,
      None => try!(self.read_inode(entry.ino as u64)).file_type,
    };

    handle.offset = handle.offset + entry.rec_len as u64;
    Ok(Some(DirLine {
      ino: entry.ino as u64,
      file_type: file_type,
      name: entry.name 
    }))
  }

  pub fn dir_close(&mut self, _handle: DirHandle) -> Result<()> {
    Ok(())
  }

  pub fn file_open(&mut self, inode: Inode) -> Result<FileHandle> {
    if inode.file_type == FileType::Regular {
      Ok(FileHandle { inode: inode })
    } else {
      Err(Error::new(format!("inode is not a regular file")))
    }
  }

  pub fn file_read(&mut self, handle: &mut FileHandle,
      offset: u64, buffer: &mut [u8]) -> Result<u64> 
  {
    self.read_file_data(&handle.inode, offset, buffer)
  }

  pub fn file_write(&mut self, handle: &mut FileHandle,
      offset: u64, buffer: &[u8]) -> Result<u64>
  {
    self.write_file_data(&mut handle.inode, offset, buffer)
  }

  pub fn file_close(&mut self, _handle: FileHandle) -> Result<()> {
    Ok(())
  }

  pub fn link_read(&mut self, inode: Inode) -> Result<Vec<u8>> {
    if inode.file_type == FileType::Symlink {
      let fast_symlink =
        if inode.file_acl != 0 {
          inode.size_512 as u64 == self.block_size() / 512 
        } else {
          inode.size_512 == 0
        };
      let mut buffer = make_buffer(inode.size + 4);

      let length = 
        if fast_symlink {
          for i in 0..cmp::min(inode.block.len(), inode.size as usize / 4 + 1) {
            encode::encode_u32(inode.block[i], &mut buffer[4*i..]);
          }
          inode.size
        } else {
          try!(self.read_file_data(&inode, 0, &mut buffer[..]))
        };
      buffer.truncate(length as usize);
      Ok(buffer)
    } else {
      Err(Error::new(format!("inode is not a symlink")))
    }
  }

  fn read_file_data(&mut self, inode: &Inode, 
    offset: u64, buffer: &mut [u8]) -> Result<u64> 
  {
    let block_size = self.block_size();
    let max_length = cmp::min(buffer.len() as u64, inode.size - offset);
    let mut chunk_begin = 0;
    while chunk_begin < max_length {
      let chunk_block = (offset + chunk_begin) / block_size;
      let chunk_offset = (offset + chunk_begin) % block_size;
      let chunk_length = cmp::min(max_length - chunk_begin,
          block_size - chunk_offset);
      try!(self.read_file_block(inode, chunk_block, chunk_offset,
            &mut (buffer[chunk_begin as usize..])[0..chunk_length as usize]));
      chunk_begin = chunk_begin + chunk_length;
    }
    Ok(chunk_begin)
  }

  fn read_file_block(&mut self, inode: &Inode, file_block: u64,
    offset: u64, buffer: &mut [u8]) -> Result<()>
  {
    assert!(offset + buffer.len() as u64 <= self.block_size());
    let real_block = match try!(self.get_file_block(inode, file_block)) {
      Some(block) => block,
      None => return Err(Error::new(
            format!("File block {} is out of the allocated range", file_block))),
    };
    let block_offset = real_block * self.block_size() + offset;
    self.volume.read(block_offset, buffer)
  }

  fn read_indirect(&mut self, indirect_block: u64, entry: u64) -> Result<u64> {
    let mut buffer = [0; 4];
    let entry_offset = indirect_block * self.block_size() + entry * 4;
    assert!(entry < self.block_size() / 4);
    try!(self.volume.read(entry_offset, &mut buffer[..]));
    Ok(decode::decode_u32(&buffer[..]) as u64)
  }

  fn write_indirect(&mut self, indirect_block: u64, entry: u64, link: u64) -> Result<()> {
    let mut buffer = [0; 4];
    let entry_offset = indirect_block * self.block_size() + entry * 4;
    assert!(entry < self.block_size() / 4);
    encode::encode_u32(link as u32, &mut buffer[..]);
    self.volume.write(entry_offset, &buffer[..])
  }

  fn write_file_data(&mut self, inode: &mut Inode,
    offset: u64, buffer: &[u8]) -> Result<u64>
  {
    println!(".write_file_data(ino {}, offset {}, len {})",
      inode.ino, offset, buffer.len());
    let block_size = self.block_size();
    let mut chunk_begin = 0;
    while chunk_begin < buffer.len() as u64 {
      let chunk_block = (offset + chunk_begin) / block_size;
      let chunk_offset = (offset + chunk_begin) % block_size;
      let chunk_length = cmp::min(buffer.len() as u64 - chunk_begin,
          block_size - chunk_offset);
      try!(self.write_file_block(inode, chunk_block, chunk_offset,
            &(buffer[chunk_begin as usize..])[0..chunk_length as usize]));
      chunk_begin = chunk_begin + chunk_length;
    }

    if inode.size < offset + chunk_begin {
      inode.size = offset + chunk_begin;
      try!(self.write_inode(inode));
    }

    Ok(chunk_begin)
  }

  fn write_file_block(&mut self, inode: &mut Inode, file_block: u64,
    offset: u64, buffer: &[u8]) -> Result<()>
  {
    println!(".write_file_block(ino {}, file_block {}, offset {}, len {})",
      inode.ino, file_block, offset, buffer.len());
    assert!(offset + buffer.len() as u64 <= self.block_size());
    let real_block = match try!(self.get_file_block(inode, file_block)) {
      Some(block) => block,
      None => try!(self.alloc_file_block(inode, file_block)),
    };
    println!("write ino {}, file_block {} => block {}",
             inode.ino, file_block, real_block);
    let block_offset = real_block * self.block_size() + offset;
    self.volume.write(block_offset, buffer)
  }

  fn alloc_file_block(&mut self, inode: &mut Inode, file_block: u64) 
    -> Result<u64> 
  {
    println!(".alloc_file_block(ino {}, file_block {})", inode.ino, file_block);
    let (inode_group_idx, _) = self.get_ino_group(inode.ino);
    let block = match try!(self.alloc_block(inode_group_idx)) {
      Some(block) => block,
      None => return Err(Error::new(format!("No free blocks remain for files"))),
    };
    try!(self.set_file_block(inode, file_block, block));
    Ok(block)
  }

  fn alloc_indirect_block(&mut self, inode: &mut Inode) -> Result<u64> {
    println!(".alloc_indirect_block(ino {})", inode.ino);
    let (inode_group_idx, _) = self.get_ino_group(inode.ino);
    let block = match try!(self.alloc_block(inode_group_idx)) {
      Some(block) => block,
      None => return Err(Error::new(
          format!("No free blocks remain for indirections"))),
    };

    let zeros = make_buffer(self.block_size());
    let offset = block * self.block_size();
    try!(self.volume.write(offset, &zeros[..]));
    Ok(block)
  }

  fn alloc_block(&mut self, first_group_idx: u64) -> Result<Option<u64>> {
    Ok(match try!(self.alloc_group_block(first_group_idx)) {
      Some(block) => {
        println!("alloc_block in first_group {}: {}", first_group_idx, block);
        Some(self.group_local_to_block(first_group_idx, block))
      },
      None => {
        let group_count = self.group_count();
        for group_idx in (first_group_idx..group_count).chain(0..first_group_idx) {
          if let Some(block) = try!(self.alloc_group_block(group_idx)) {
            println!("alloc_block in group {}: {}", group_idx, block);
            return Ok(Some(self.group_local_to_block(group_idx, block)));
          }
        }
        None
      }
    })
  }

  fn alloc_group_block(&mut self, group_idx: u64) -> Result<Option<u64>> {
    let group_desc = try!(self.read_group_desc(group_idx));
    let block_size = self.block_size();
    let blocks_per_group = self.superblock.blocks_per_group as u64;

    let mut bitmap_buffer = make_buffer(block_size);
    let mut bitmap_block = 0;
    while bitmap_block * block_size < blocks_per_group / 8 {
      let bitmap_offset = (bitmap_block + group_desc.block_bitmap as u64) * block_size;
      let bitmap_begin = bitmap_block * block_size;
      let bitmap_end = cmp::min(bitmap_begin + block_size, blocks_per_group / 8);

      let bitmap = &mut bitmap_buffer[0..(bitmap_end - bitmap_begin) as usize];
      try!(self.volume.read(bitmap_offset, bitmap));

      println!("group {}, bitmap block {} ({}..{}):\n{:?}",
        group_idx, bitmap_block, bitmap_begin, bitmap_end, bitmap);

      for byte in 0..bitmap.len() as u64 {
        if bitmap[byte as usize] == 0xff {
          continue
        }

        for bit in 0..8 {
          if (bitmap[byte as usize] & (1 << bit)) == 0 {
            println!("alloc group {} block {}",
                     group_idx, (bitmap_begin + byte) * 8 + bit);
            try!(self.volume.write(bitmap_offset + byte,
                  &[bitmap[byte as usize] | (1 << bit)]));
            return Ok(Some((bitmap_begin + byte) * 8 + bit))
          }
        }
      }

      bitmap_block = bitmap_block + 1;
    }

    Ok(None)
  }

  fn get_ino_group(&self, ino: u64) -> (u64, u64) {
    let group_size = self.superblock.inodes_per_group as u64;
    ((ino - 1) / group_size, (ino - 1) % group_size)
  }

  fn get_file_block(&mut self, inode: &Inode, file_block: u64) -> Result<Option<u64>> {
    Ok(Some(match self.file_block_to_pos(file_block) {
      BlockPos::Level0(level0) => {
        let block0 = inode.block[level0 as usize] as u64;
        if block0 == 0 { return Ok(None) } else { block0 }
      },
      BlockPos::Level1(level0) => {
        let block1 = inode.block[12] as u64;
        if block1 == 0 { return Ok(None) }
        let block0 = try!(self.read_indirect(block1, level0));
        if block0 == 0 { return Ok(None) } else { block0 }
      },
      BlockPos::Level2(level1, level0) => {
        let block2 = inode.block[13] as u64;
        if block2 == 0 { return Ok(None) }
        let block1 = try!(self.read_indirect(block2, level1));
        if block1 == 0 { return Ok(None) }
        let block0 = try!(self.read_indirect(block1, level0));
        if block0 == 0 { return Ok(None) } else { block0 }
      },
      BlockPos::Level3(level2, level1, level0) => {
        let block3 = inode.block[14] as u64;
        if block3 == 0 { return Ok(None) }
        let block2 = try!(self.read_indirect(block3, level2));
        if block2 == 0 { return Ok(None) }
        let block1 = try!(self.read_indirect(block2, level1));
        if block1 == 0 { return Ok(None) }
        let block0 = try!(self.read_indirect(block1, level0));
        if block0 == 0 { return Ok(None) } else { block0 }
      },
      BlockPos::OutOfRange =>
        return Err(Error::new(
          format!("File block {} is out of range for reading", file_block))),
    }))
  }

  fn set_file_block(&mut self, inode: &mut Inode,
    file_block: u64, block: u64) -> Result<()> 
  {
    println!(".set_file_block(ino {}, file_block {}, block {})",
      inode.ino, file_block, block);

    if let Some(prev_block) = try!(self.get_file_block(inode, file_block)) {
      panic!("inode {}, file block {}: tried to overwrite block {} with {}",
             inode.ino, file_block, prev_block, block);
    }

    let inode_indirect = |fs: &mut Filesystem, inode: &mut Inode,
      idx: u64| -> Result<_> 
    {
      if inode.block[idx as usize] == 0 {
        inode.block[idx as usize] = try!(fs.alloc_indirect_block(inode)) as u32;
        try!(fs.write_inode(inode));
      }
      Ok(inode.block[idx as usize] as u64)
    };

    let block_indirect = |fs: &mut Filesystem, inode: &mut Inode,
      indirect: u64, entry: u64| -> Result<_> 
    {
      let old_block = try!(fs.read_indirect(indirect, entry));
      if old_block == 0 {
        let new_block = try!(fs.alloc_indirect_block(inode));
        try!(fs.write_indirect(indirect, entry, new_block));
        Ok(new_block)
      } else {
        Ok(old_block)
      }
    };

    match self.file_block_to_pos(file_block) {
      BlockPos::Level0(level0) => {
        inode.block[level0 as usize] = block as u32;
        try!(self.write_inode(inode));
      },
      BlockPos::Level1(level0) => {
        let block1 = try!(inode_indirect(self, inode, 12));
        try!(self.write_indirect(block1, level0, block));
      },
      BlockPos::Level2(level1, level0) => {
        let block2 = try!(inode_indirect(self, inode, 13));
        let block1 = try!(block_indirect(self, inode, block2, level1));
        try!(self.write_indirect(block1, level0, block));
      },
      BlockPos::Level3(level2, level1, level0) => {
        let block3 = try!(inode_indirect(self, inode, 14));
        let block2 = try!(block_indirect(self, inode, block3, level2));
        let block1 = try!(block_indirect(self, inode, block2, level1));
        try!(self.write_indirect(block1, level0, block));
      },
      BlockPos::OutOfRange =>
        return Err(Error::new(
            format!("File block {} is out of range for writing", file_block))),
    }

    let min_size_512 = ((file_block + 1) * self.block_size() / 512) as u32;
    if inode.size_512 < min_size_512 {
      inode.size_512 = min_size_512;
      self.write_inode(inode)
    } else {
      Ok(())
    }
  }

  fn file_block_to_pos(&self, file_block: u64) -> BlockPos {
    let indirect_1_size: u64 = self.block_size() / 4;
    let indirect_2_size = indirect_1_size * indirect_1_size;
    let indirect_3_size = indirect_1_size * indirect_2_size;
    if file_block < 12 {
      BlockPos::Level0(file_block)
    } else if file_block < 12 + indirect_1_size {
      BlockPos::Level1(file_block - 12)
    } else if file_block < 12 + indirect_1_size + indirect_2_size {
      BlockPos::Level2((file_block - 12) / indirect_1_size,
        (file_block - 12) % indirect_1_size)
    } else if file_block < 12 + indirect_1_size + indirect_2_size + indirect_3_size {
      BlockPos::Level3((file_block - 12) / indirect_2_size,
        ((file_block - 12) % indirect_2_size) / indirect_1_size,
        ((file_block - 12) % indirect_2_size) % indirect_1_size)
    } else {
      BlockPos::OutOfRange
    }
  }

  fn group_local_to_block(&self, group_idx: u64, local_block: u64) -> u64 {
    group_idx * self.superblock.blocks_per_group as u64 +
      self.superblock.first_data_block as u64 + local_block
  }
}

fn make_buffer(size: u64) -> Vec<u8> {
  iter::repeat(0).take(size as usize).collect()
}
