use std::{cmp, iter};
use defs::*;
use decode;
use encode;
use error::{Error, Result};
use read_raw::{ReadRaw};

pub struct Filesystem {
  reader: Box<ReadRaw>,
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

  pub fn new(mut reader: Box<ReadRaw>) -> Result<Filesystem> {
    let mut superblock_buf = make_buffer(1024);
    try!(reader.read(1024, &mut superblock_buf[..]));
    let superblock = try!(decode::decode_superblock(&superblock_buf[..], true));
    Ok(Filesystem { reader: reader, superblock: superblock })
  }

  pub fn read_inode(&mut self, ino: u64) -> Result<Inode> {
    let group_size = self.superblock.inodes_per_group as u64;
    let inode_size = self.superblock.inode_size as u64;
    let (group_idx, local_idx) = ((ino - 1) / group_size, (ino - 1) % group_size);
    let group_desc = try!(self.read_group_desc(group_idx));
    let offset = group_desc.inode_table as u64 * self.block_size() 
        + local_idx * inode_size;

    let mut inode_buf = make_buffer(inode_size);
    try!(self.reader.read(offset, &mut inode_buf[..]));
    decode::decode_inode(&self.superblock, &inode_buf[..])
  }

  fn read_group_desc(&mut self, group_idx: u64) -> Result<GroupDesc> {
    let group_desc_block = self.superblock.first_data_block as u64 + 1;
    let offset = group_desc_block * self.block_size() + group_idx * 32;
    let mut desc_buf = make_buffer(32);
    try!(self.reader.read(offset, &mut desc_buf[..]));
    decode::decode_group_desc(&self.superblock, &desc_buf[..])
  }

  fn block_size(&self) -> u64 {
    1024 << self.superblock.log_block_size 
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
            encode::encode_u32(&mut buffer[4*i..], inode.block[i]);
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
            &mut (buffer[chunk_begin as usize ..])[0..chunk_length as usize]));
      chunk_begin = chunk_begin + chunk_length;
    }
    Ok(chunk_begin)
  }

  fn read_file_block(&mut self, inode: &Inode, file_block: u64,
    offset: u64, buffer: &mut [u8]) -> Result<()>
  {
    assert!(offset + buffer.len() as u64 <= self.block_size());

    let real_block = match self.file_block_to_pos(file_block) {
      BlockPos::Level0(level0) =>
        inode.block[level0 as usize] as u64,
      BlockPos::Level1(level0) => {
        let block1 = inode.block[12] as u64;
        try!(self.read_indirect(block1, level0))
      },
      BlockPos::Level2(level1, level0) => {
        let block2 = inode.block[12] as u64;
        let block1 = try!(self.read_indirect(block2, level1));
        try!(self.read_indirect(block1, level0))
      },
      BlockPos::Level3(level2, level1, level0) => {
        let block3 = inode.block[14] as u64;
        let block2 = try!(self.read_indirect(block3, level2));
        let block1 = try!(self.read_indirect(block2, level1));
        try!(self.read_indirect(block1, level0))
      },
      BlockPos::OutOfRange =>
        return Err(Error::new(
            format!("File block {} is out of available range", file_block))),
    };

    let block_offset = real_block * self.block_size() + offset;
    self.reader.read(block_offset, buffer)
  }

  fn read_indirect(&mut self, indirect_block: u64, entry: u64) -> Result<u64> {
    let mut buffer = [0; 4];
    let entry_offset = indirect_block * self.block_size() + entry * 4;
    assert!(entry < self.block_size() / 4);
    try!(self.reader.read(entry_offset, &mut buffer[..]));
    Ok(decode::decode_u32(&buffer[..]) as u64)
  }

  fn file_block_to_pos(&mut self, file_block: u64) -> BlockPos {
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
}

fn make_buffer(size: u64) -> Vec<u8> {
  iter::repeat(0).take(size as usize).collect()
}
