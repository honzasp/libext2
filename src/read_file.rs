use std::{cmp};
use context::{Context};
use error::{Result, Error};
use inode::{Inode};
use read_int::{read_u32};

#[derive(Copy, Clone, Debug)]
enum BlockPos {
  Level0(u64),
  Level1(u64),
  Level2(u64, u64),
  Level3(u64, u64, u64),
  OutOfRange,
}

pub fn read(ctx: &Context, inode: &Inode, 
  offset: u64, buffer: &mut [u8]) -> Result<u64> 
{
  let block_size = ctx.block_size();
  let max_length = cmp::min(buffer.len() as u64, inode.size - offset);
  let mut chunk_begin = 0;
  while chunk_begin < max_length {
    let chunk_block = (offset + chunk_begin) / block_size;
    let chunk_offset = (offset + chunk_begin) % block_size;
    let chunk_length = cmp::min(max_length - chunk_begin,
        block_size - chunk_offset);
    try!(read_block(ctx, inode, chunk_block, chunk_offset,
          &mut (buffer[chunk_begin as usize ..])[0..chunk_length as usize]));
    chunk_begin = chunk_begin + chunk_length;
  }
  Ok(chunk_begin)
}

pub fn read_block(ctx: &Context, inode: &Inode, file_block: u64,
  offset: u64, buffer: &mut [u8]) -> Result<()>
{
  let block_size = ctx.block_size();
  assert!(offset + buffer.len() as u64 <= block_size);

  let real_block = match file_block_to_pos(ctx, file_block) {
    BlockPos::Level0(level0) =>
      inode.block[level0 as usize] as u64,
    BlockPos::Level1(level0) => {
      let block1 = inode.block[12] as u64;
      try!(read_indirect(ctx, block1, level0))
    },
    BlockPos::Level2(level1, level0) => {
      let block2 = inode.block[12] as u64;
      let block1 = try!(read_indirect(ctx, block2, level1));
      try!(read_indirect(ctx, block1, level0))
    },
    BlockPos::Level3(level2, level1, level0) => {
      let block3 = inode.block[14] as u64;
      let block2 = try!(read_indirect(ctx, block3, level2));
      let block1 = try!(read_indirect(ctx, block2, level1));
      try!(read_indirect(ctx, block1, level0))
    },
    BlockPos::OutOfRange =>
      return Err(Error::new(
          format!("File block {} is out of available range", file_block))),
  };

  ctx.read(real_block * ctx.block_size() + offset, buffer)
}

fn read_indirect(ctx: &Context, indirect_block: u64, entry: u64) -> Result<u64> {
  let mut buffer = [0; 4];
  let entry_offset = indirect_block * ctx.block_size() + entry * 4;
  assert!(entry < ctx.block_size() / 4);
  try!(ctx.read(entry_offset, &mut buffer[..]));
  Ok(read_u32(&buffer[..]) as u64)
}

fn file_block_to_pos(ctx: &Context, file_block: u64) -> BlockPos {
  let indirect_1_size: u64 = ctx.block_size() / 4;
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
