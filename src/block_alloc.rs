use prelude::*;

pub fn alloc_block(fs: &mut Filesystem, first_group_idx: u64) -> Result<Option<u64>> {
  Ok(match try!(alloc_block_in_group(fs, first_group_idx)) {
    Some(block) =>
      Some(group_local_to_block(fs, first_group_idx, block)),
    None => {
      let group_count = fs.group_count();
      for group_idx in (first_group_idx..group_count).chain(0..first_group_idx) {
        if let Some(block) = try!(alloc_block_in_group(fs, group_idx)) {
          return Ok(Some(group_local_to_block(fs, group_idx, block)));
        }
      }
      None
    }
  })
}

fn alloc_block_in_group(fs: &mut Filesystem, group_idx: u64) -> Result<Option<u64>> {
  let group_id = group_idx as usize;
  if fs.groups[group_id].desc.free_blocks_count == 0 {
    return Ok(None)
  }

  match find_zero_bit_in_bitmap(&fs.groups[group_id].block_bitmap[..]) {
    Some((byte, bit)) => {
      fs.groups[group_id].block_bitmap[byte as usize] |= 1 << bit;
      fs.groups[group_id].desc.free_blocks_count -= 1;
      fs.groups[group_id].dirty = true;
      fs.superblock.free_blocks_count -= 1;
      fs.superblock_dirty = true;
      Ok(Some(byte * 8 + bit))
    },
    None => Ok(None),
  }
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
