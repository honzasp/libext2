use prelude::*;

pub fn alloc_block(fs: &mut Filesystem, first_group_idx: u64) -> Result<Option<u64>> {
  alloc(fs, first_group_idx, alloc_block_in_group)
}

pub fn alloc_inode(fs: &mut Filesystem, first_group_idx: u64) -> Result<Option<u64>> {
  alloc(fs, first_group_idx, alloc_inode_in_group)
}

pub fn dealloc_block(fs: &mut Filesystem, block: u64) -> Result<()> {
  let (group_idx, local_idx) = get_block_group(fs, block);
  let group_id = group_idx as usize;
  let (local_byte, local_bit) = (local_idx / 8, local_idx % 8);
  fs.groups[group_id].desc.free_blocks_count += 1;
  fs.groups[group_id].block_bitmap[local_byte as usize] &= !(1 << local_bit);
  fs.groups[group_id].dirty = true;
  fs.superblock.free_blocks_count += 1;
  fs.superblock_dirty = true;
  Ok(())
}

pub fn dealloc_inode(fs: &mut Filesystem, ino: u64) -> Result<()> {
  let (group_idx, local_idx) = get_ino_group(fs, ino);
  let group_id = group_idx as usize;
  let (local_byte, local_bit) = (local_idx / 8, local_idx % 8);
  fs.groups[group_id].desc.free_inodes_count += 1;
  fs.groups[group_id].inode_bitmap[local_byte as usize] &= !(1 << local_bit);
  fs.groups[group_id].dirty = true;
  fs.superblock.free_inodes_count += 1;
  fs.superblock_dirty = true;
  Ok(())
}

fn alloc(fs: &mut Filesystem, first_group_idx: u64,
  alloc_in_group: fn(&mut Filesystem, u64) -> Result<Option<u64>>) 
  -> Result<Option<u64>>
{
  Ok(match try!(alloc_in_group(fs, first_group_idx)) {
    Some(resource) => Some(resource),
    None => {
      let group_count = fs.group_count();
      for group_idx in (first_group_idx..group_count).chain(0..first_group_idx) {
        if let Some(resource) = try!(alloc_in_group(fs, group_idx)) {
          return Ok(Some(resource));
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
      Ok(Some(group_idx * fs.superblock.blocks_per_group as u64 +
              fs.superblock.first_data_block as u64 +
              byte * 8 + bit))
    },
    None => Ok(None),
  }
}

fn alloc_inode_in_group(fs: &mut Filesystem, group_idx: u64) -> Result<Option<u64>> {
  let group_id = group_idx as usize;
  if fs.groups[group_id].desc.free_inodes_count == 0 {
    return Ok(None)
  }

  match find_zero_bit_in_bitmap(&fs.groups[group_id].inode_bitmap[..]) {
    Some((byte, bit)) => {
      fs.groups[group_id].inode_bitmap[byte as usize] |= 1 << bit;
      fs.groups[group_id].desc.free_inodes_count -= 1;
      fs.groups[group_id].dirty = true;
      fs.superblock.free_inodes_count -= 1;
      fs.superblock_dirty = true;
      Ok(Some(group_idx * fs.superblock.inodes_per_group as u64 + 
              8 * byte + bit + 1))
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
