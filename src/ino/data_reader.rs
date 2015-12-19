use std::{cmp};
use error::{Error, Result};
use ino::{Context, Inode};
use ino::integer::{read_u32};

pub struct DataReader<'c> {
  ctx: &'c Context,
  block: [u32; 15],
}

#[derive(Copy, Clone, Debug)]
enum BlockPos {
  Level0(u64),
  Level1(u64),
  Level2(u64, u64),
  Level3(u64, u64, u64),
  OutOfRange,
}

impl<'c> DataReader<'c> {
  pub fn new(ctx: &'c Context, inode: &Inode) -> DataReader<'c> {
    DataReader { ctx: ctx, block: inode.block }
  }

  pub fn read(&mut self, offset: u64, buffer: &mut [u8]) -> Result<()> {
    let block_size = self.ctx.block_size();
    let mut chunk_begin = 0;
    while chunk_begin < buffer.len() as u64 {
      let chunk_block = (offset + chunk_begin) / block_size;
      let chunk_offset = (offset + chunk_begin) % block_size;
      let chunk_length = cmp::min(buffer.len() as u64 - chunk_begin,
          block_size - chunk_offset);
      try!(self.read_file_block(chunk_block, chunk_offset,
            &mut (buffer[chunk_begin as usize ..])[0..chunk_length as usize]));
      chunk_begin = chunk_begin + chunk_length;
    }
    Ok(())
  }

  pub fn read_file_block(&mut self, file_block: u64, offset: u64, buffer: &mut [u8])
    -> Result<()>
  {
    assert!(offset + buffer.len() as u64 <= self.ctx.block_size());

    let real_block = match self.file_block_to_pos(file_block) {
      BlockPos::Level0(level0) =>
        self.block[level0 as usize] as u64,
      BlockPos::Level1(level0) => {
        let block1 = self.block[12] as u64;
        try!(self.read_indirect(block1, level0))
      },
      BlockPos::Level2(level1, level0) => {
        let block2 = self.block[12] as u64;
        let block1 = try!(self.read_indirect(block2, level1));
        try!(self.read_indirect(block1, level0))
      },
      BlockPos::Level3(level2, level1, level0) => {
        let block3 = self.block[14] as u64;
        let block2 = try!(self.read_indirect(block3, level2));
        let block1 = try!(self.read_indirect(block2, level1));
        try!(self.read_indirect(block1, level0))
      },
      BlockPos::OutOfRange =>
        return Err(Error::new(
            format!("File block {} is out of available range", file_block))),
    };

    self.ctx.read(real_block * self.ctx.block_size() + offset, buffer)
  }

  fn read_indirect(&mut self, indirect_block: u64, entry: u64) -> Result<u64> {
    let mut buffer = [0; 4];
    let entry_offset = indirect_block * self.ctx.block_size() + entry * 4;
    assert!(entry < self.ctx.block_size() / 4);
    try!(self.ctx.read(entry_offset, &mut buffer[..]));
    Ok(read_u32(&buffer[..]) as u64)
  }

  fn file_block_to_pos(&self, file_block: u64) -> BlockPos {
    let indirect_1_size: u64 = self.ctx.block_size() / 4;
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
