use std::{cmp};
use prelude::*;

pub fn alloc_block(fs: &mut Filesystem, first_group_idx: u64) -> Result<Option<u64>> {
  Ok(match try!(alloc_block_in_group(fs, first_group_idx)) {
    Some(block) => {
      println!("alloc_block in first_group {}: {}", first_group_idx, block);
      Some(group_local_to_block(fs, first_group_idx, block))
    },
    None => {
      let group_count = fs.group_count();
      for group_idx in (first_group_idx..group_count).chain(0..first_group_idx) {
        if let Some(block) = try!(alloc_block_in_group(fs, group_idx)) {
          println!("alloc_block in group {}: {}", group_idx, block);
          return Ok(Some(group_local_to_block(fs, group_idx, block)));
        }
      }
      None
    }
  })
}

fn alloc_block_in_group(fs: &mut Filesystem, group_idx: u64) -> Result<Option<u64>> {
  let group_desc = try!(read_group_desc(fs, group_idx));
  let block_size = fs.block_size();
  let blocks_per_group = fs.superblock.blocks_per_group as u64;

  let mut bitmap_buffer = make_buffer(block_size);
  let mut bitmap_block = 0;
  while bitmap_block * block_size < blocks_per_group / 8 {
    let bitmap_offset = (bitmap_block + group_desc.block_bitmap as u64) * block_size;
    let bitmap_begin = bitmap_block * block_size;
    let bitmap_end = cmp::min(bitmap_begin + block_size, blocks_per_group / 8);

    let bitmap = &mut bitmap_buffer[0..(bitmap_end - bitmap_begin) as usize];
    try!(fs.volume.read(bitmap_offset, bitmap));

    match find_zero_bit_in_bitmap(bitmap) {
      Some((byte, bit)) => {
        println!("alloc group {} block {}",
                  group_idx, (bitmap_begin + byte) * 8 + bit);
        try!(fs.volume.write(bitmap_offset + byte,
              &[bitmap[byte as usize] | (1 << bit)]));
        return Ok(Some((bitmap_begin + byte) * 8 + bit))
      },
      None => {},
    }
    bitmap_block = bitmap_block + 1;
  }

  Ok(None)
}

fn find_zero_bit_in_bitmap(bitmap: &[u8]) -> Option<(u64, u64)> {
  for byte in 0..bitmap.len() as u64 {
    if bitmap[byte as usize] == 0xff {
      continue
    }

    for bit in 0..8 {
      if (bitmap[byte as usize] & (1 << bit)) == 0 {
        return Some((byte, bit))
      }
    }
  }
  None
}

fn group_local_to_block(fs: &Filesystem, group_idx: u64, local_block: u64) -> u64 {
  group_idx * fs.superblock.blocks_per_group as u64 +
    fs.superblock.first_data_block as u64 + local_block
}
