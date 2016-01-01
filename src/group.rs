use prelude::*;

pub fn read_group(fs: &mut Filesystem, group_idx: u64) -> Result<Group> {
  let table_block = fs.superblock.first_data_block as u64 + 1;
  let desc = try!(read_group_desc(fs, table_block, group_idx));

  let block_bitmap_offset = desc.block_bitmap as u64 * fs.block_size();
  let mut block_bitmap = make_buffer(fs.superblock.blocks_per_group as u64 / 8);
  try!(fs.volume.read(block_bitmap_offset, &mut block_bitmap[..]));

  let inode_bitmap_offset = desc.inode_bitmap as u64 * fs.block_size();
  let mut inode_bitmap = make_buffer(fs.superblock.inodes_per_group as u64 / 8);
  try!(fs.volume.read(inode_bitmap_offset, &mut inode_bitmap[..]));

  Ok(Group {
    idx: group_idx,
    desc: desc,
    block_bitmap: block_bitmap,
    inode_bitmap: inode_bitmap,
    dirty: false 
  })
}

fn read_group_desc(fs: &mut Filesystem, table_block: u64,
  group_idx: u64) -> Result<GroupDesc> 
{
  let offset = table_block * fs.block_size() + group_idx * 32;
  let mut desc_buf = make_buffer(32);
  try!(fs.volume.read(offset, &mut desc_buf[..]));
  decode_group_desc(&fs.superblock, &desc_buf[..])
}

fn write_group(fs: &mut Filesystem, group_idx: u64) -> Result<()> {
  let group_desc = fs.groups[group_idx as usize].desc;
  let table_block = fs.superblock.first_data_block as u64 + 1;
  try!(write_group_desc(fs, table_block, group_idx, &group_desc));

  let block_bitmap_offset = group_desc.block_bitmap as u64 * fs.block_size();
  try!(fs.volume.write(block_bitmap_offset,
    &fs.groups[group_idx as usize].block_bitmap[..]));

  let inode_bitmap_offset = group_desc.inode_bitmap as u64 * fs.block_size();
  try!(fs.volume.write(inode_bitmap_offset,
    &fs.groups[group_idx as usize].inode_bitmap[..]));

  Ok(())
}

fn write_group_desc(fs: &mut Filesystem, table_block: u64,
  group_idx: u64, desc: &GroupDesc) -> Result<()> 
{
  let offset = table_block * fs.block_size() + group_idx * 32;
  let mut desc_buf = make_buffer(32);
  try!(fs.volume.read(offset, &mut desc_buf[..]));
  try!(encode_group_desc(&fs.superblock, desc, &mut desc_buf[..]));
  fs.volume.write(offset, &desc_buf[..])
}

pub fn flush_group(fs: &mut Filesystem, group_idx: u64) -> Result<()> {
  if fs.groups[group_idx as usize].dirty {
    try!(write_group(fs, group_idx));
    fs.groups[group_idx as usize].dirty = false;
  }
  Ok(())
}

pub fn get_ino_group(fs: &Filesystem, ino: u64) -> (u64, u64) {
  let group_size = fs.superblock.inodes_per_group as u64;
  ((ino - 1) / group_size, (ino - 1) % group_size)
}

pub fn get_block_group(fs: &Filesystem, block: u64) -> (u64, u64) {
  let group_size = fs.superblock.blocks_per_group as u64;
  let rel_block = block - fs.superblock.first_data_block as u64;
  (rel_block / group_size, rel_block % group_size)
}
