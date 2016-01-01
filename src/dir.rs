use std::{cmp};
use prelude::*;

#[derive(Debug, Copy, Clone)]
pub struct DirHandle {
  ino: u64,
  offset: u64,
}

#[derive(Debug)]
pub struct DirLine {
  pub ino: u64,
  pub file_type: FileType,
  pub name: Vec<u8>,
}

pub fn lookup_in_dir(fs: &mut Filesystem, dir_ino: u64, name: &[u8]) 
  -> Result<Option<u64>> 
{
  let dir_inode = try!(get_inode(fs, dir_ino));
  if dir_inode.mode.file_type != FileType::Dir {
    return Err(Error::new(format!("inode {} is not a directory", dir_ino)))
  }

  let mut offset = 0;
  while offset < dir_inode.size {
    let (entry, entry_name, next_offset) = try!(read_dir_entry(fs, &dir_inode, offset));
    if entry.ino != 0 && name == &entry_name[..] {
      return Ok(Some(entry.ino as u64))
    }
    offset = next_offset;
  }
  Ok(None)
}

pub fn remove_from_dir(fs: &mut Filesystem, dir_ino: u64, name: &[u8])
  -> Result<bool>
{
  let mut dir_inode = try!(get_inode(fs, dir_ino));
  if dir_inode.mode.file_type != FileType::Dir {
    return Err(Error::new(format!("inode {} is not a directory", dir_ino)))
  }

  let mut offset = 0;
  let mut prev_offset = 0;
  while offset < dir_inode.size {
    let (entry, entry_name, next_offset) = try!(read_dir_entry(fs, &dir_inode, offset));
    if entry.ino != 0 && name == &entry_name[..] {
      let mut entry_inode = try!(get_inode(fs, entry.ino as u64));
      try!(unlink_inode(fs, &mut entry_inode));
      try!(erase_dir_entry(fs, &mut dir_inode, offset, prev_offset, next_offset));
      return Ok(true);
    }
    prev_offset = offset;
    offset = next_offset;
  }

  Ok(false)
}

pub fn move_between_dirs(fs: &mut Filesystem,
  source_dir_ino: u64, source_name: &[u8],
  target_dir_ino: u64, target_name: &[u8]) -> Result<bool>
{
  let mut source_dir_inode = try!(get_inode(fs, source_dir_ino));
  let mut target_dir_inode = try!(get_inode(fs, target_dir_ino));

  if source_dir_inode.mode.file_type != FileType::Dir {
    return Err(Error::new(format!("source inode {} is not a directory", source_dir_ino)))
  } else if target_dir_inode.mode.file_type != FileType::Dir {
    return Err(Error::new(format!("target inode {} is not a directory", target_dir_ino)))
  }

  let mut offset = 0;
  let mut prev_offset = 0;
  while offset < source_dir_inode.size {
    let (entry, entry_name, next_offset) =
      try!(read_dir_entry(fs, &source_dir_inode, offset));
    if entry.ino != 0 && source_name == &entry_name[..] {
      let mut entry_inode = try!(get_inode(fs, entry.ino as u64));
      try!(add_dir_entry(fs, &mut target_dir_inode, &mut entry_inode, target_name));
      try!(unlink_inode(fs, &mut entry_inode));
      try!(erase_dir_entry(fs, &mut source_dir_inode, offset, prev_offset, next_offset));
      return Ok(true);
    }
    prev_offset = offset;
    offset = next_offset;
  }

  Ok(false)
}

pub fn open_dir(fs: &mut Filesystem, ino: u64) -> Result<DirHandle> {
  let inode = try!(get_inode(fs, ino));
  if inode.mode.file_type == FileType::Dir {
    Ok(DirHandle { ino: ino, offset: 0 })
  } else {
    return Err(Error::new(format!("inode is not a directory")))
  }
}

pub fn read_dir(fs: &mut Filesystem, mut handle: DirHandle) 
  -> Result<Option<(DirHandle, DirLine)>> 
{
  let inode = try!(get_inode(fs, handle.ino));
  if handle.offset >= inode.size {
    return Ok(None)
  }

  loop {
    let (entry, name, next_offset) = try!(read_dir_entry(fs, &inode, handle.offset));
    let file_type = match entry.file_type {
      Some(file_type) => file_type,
      None => try!(get_inode(fs, entry.ino as u64)).mode.file_type,
    };

    handle.offset = next_offset;

    if entry.ino != 0 {
      return Ok(Some((handle, DirLine {
        ino: entry.ino as u64,
        file_type: file_type,
        name: name,
      })))
    }
  }
}

pub fn close_dir(_fs: &mut Filesystem, _handle: DirHandle) -> Result<()> {
  Ok(())
}

#[derive(Debug)]
struct FreeSpace {
  offset: u64,
  prev_offset: u64,
  next_offset: u64,
}

pub fn add_dir_entry(fs: &mut Filesystem, dir_inode: &mut Inode,
  entry_inode: &mut Inode, name: &[u8]) -> Result<()>
{
  assert_eq!(dir_inode.mode.file_type, FileType::Dir);
  let entry_size = dir_entry_size(name.len() as u64);

  let mut place_for_entry = None;
  let mut offset = 0;
  let mut last_offset = 0;
  while offset < dir_inode.size {
    let (entry, entry_name, next_offset) = try!(read_dir_entry(fs, dir_inode, offset));

    if entry_name == name {
      if entry.ino as u64 == entry_inode.ino {
        return Ok(());
      }

      let new_entry = DirEntry {
        ino: entry_inode.ino as u32,
        rec_len: entry.rec_len,
        name_len: entry.name_len,
        file_type: Some(entry_inode.mode.file_type),
      };
      try!(write_dir_entry(fs, dir_inode, offset, &new_entry, None));
      entry_inode.links_count += 1;

      let mut old_inode = try!(get_inode(fs, entry.ino as u64));
      try!(unlink_inode(fs, &mut old_inode));
      return Ok(());
    }

    let free_offset =
      align_4(if entry.ino == 0 {
        offset
      } else {
        offset + dir_entry_size(entry_name.len() as u64)
      });
    let free_size = cmp::min(next_offset - free_offset, space_in_block(fs, offset));

    if place_for_entry.is_none() && free_size >= entry_size {
      place_for_entry = Some(FreeSpace {
        offset: free_offset,
        prev_offset: offset,
        next_offset: next_offset,
      });
    }

    last_offset = offset;
    offset = next_offset;
  }

  insert_dir_entry(fs, dir_inode, entry_inode, name, place_for_entry, last_offset)
}

pub fn is_dir_empty(fs: &mut Filesystem, dir_inode: &Inode) -> Result<bool> {
  let mut offset = 0;
  while offset < dir_inode.size {
    let (entry, entry_name, next_offset) = try!(read_dir_entry(fs, &dir_inode, offset));

    if entry.ino != 0 {
      if &entry_name[..] != b"." && &entry_name[..] != b".." {
        return Ok(false);
      }
    }
    offset = next_offset;
  }
  Ok(true)
}

pub fn init_dir(fs: &mut Filesystem, parent_inode: &mut Inode, 
  dir_inode: &mut Inode) -> Result<()>
{
  let dot_dot_offset = align_4(dir_entry_size(1));
  let mut buffer = make_buffer(fs.block_size());

  let dot_entry = DirEntry {
    ino: dir_inode.ino as u32,
    rec_len: dot_dot_offset as u16,
    name_len: 1,
    file_type: Some(FileType::Dir),
  };

  let dot_dot_entry = DirEntry {
    ino: parent_inode.ino as u32,
    rec_len: (fs.block_size() - dot_dot_offset) as u16,
    name_len: 2,
    file_type: Some(FileType::Dir),
  };

  try!(encode_dir_entry(&fs.superblock, &dot_entry, &mut buffer[0..]));
  try!(encode_dir_entry(&fs.superblock, &dot_dot_entry,
    &mut buffer[dot_dot_offset as usize..]));
  buffer[(dir_entry_size(0) + 0) as usize] = b'.';
  buffer[(dot_dot_offset + dir_entry_size(0) + 0) as usize] = b'.';
  buffer[(dot_dot_offset + dir_entry_size(0) + 1) as usize] = b'.';

  try!(write_inode_data(fs, dir_inode, 0, &buffer[..]));
  parent_inode.links_count += 1;
  try!(update_inode(fs, parent_inode));
  dir_inode.links_count += 1;
  try!(update_inode(fs, dir_inode));

  let (group_idx, _) = get_ino_group(fs, dir_inode.ino);
  fs.groups[group_idx as usize].desc.used_dirs_count += 1;
  fs.groups[group_idx as usize].dirty = true;
  Ok(())
}

pub fn deinit_dir(fs: &mut Filesystem, dir_inode: &mut Inode) -> Result<()> {
  let mut dot_ino = None;
  let mut dot_dot_ino = None;

  let mut offset = 0;
  while offset < dir_inode.size {
    let (mut entry, entry_name, next_offset) =
      try!(read_dir_entry(fs, &dir_inode, offset));

    if entry.ino != 0 {
      if &entry_name[..] == b"." {
        dot_ino = Some(entry.ino as u64);
      } else if &entry_name[..] == b".." {
        dot_dot_ino = Some(entry.ino as u64);
      } else {
        return Err(Error::new(format!(
          "Cannot deinit non-empty directory {}", dir_inode.ino)));
      }

      entry.ino = 0;
      try!(write_dir_entry(fs, dir_inode, offset, &entry, None));
    }

    offset = next_offset;
  }

  match dot_ino {
    Some(ino) if ino == dir_inode.ino => 
      dir_inode.links_count -= 1,
    Some(ino) => return Err(Error::new(format!(
      "Directory {} entry '.' points to {}", dir_inode.ino, ino))),
    None => return Err(Error::new(format!(
      "Directory {} has no '.' entry", dir_inode.ino))),
  }

  match dot_dot_ino {
    Some(parent_ino) if parent_ino == dir_inode.ino =>
      return Err(Error::new(format!(
        "Directory {} entry '.' points to itself", dir_inode.ino))),
    Some(parent_ino) => {
      let mut parent_inode = try!(get_inode(fs, parent_ino));
      parent_inode.links_count -= 1;
      try!(update_inode(fs, &parent_inode));
    },
    None => return Err(Error::new(format!(
      "Directory {} has no '..' entry", dir_inode.ino))),
  }

  let (group_idx, _) = get_ino_group(fs, dir_inode.ino);
  fs.groups[group_idx as usize].desc.used_dirs_count -= 1;
  fs.groups[group_idx as usize].dirty = true;

  Ok(())
}

fn insert_dir_entry(fs: &mut Filesystem, dir_inode: &mut Inode,
  entry_inode: &mut Inode, name: &[u8],
  place_for_entry: Option<FreeSpace>, last_offset: u64) -> Result<()>
{
  let free_space = place_for_entry.unwrap_or_else(|| {
    let block_size = fs.block_size();
    let next_block = last_offset / block_size + 1;
    FreeSpace {
      offset: next_block * block_size,
      prev_offset: last_offset,
      next_offset: (next_block + 1) * block_size,
    }
  });

  let new_entry = DirEntry {
    ino: entry_inode.ino as u32,
    rec_len: (free_space.next_offset - free_space.offset) as u16,
    name_len: name.len() as u8,
    file_type: Some(entry_inode.mode.file_type),
  };
  try!(write_dir_entry(fs, dir_inode, free_space.offset, &new_entry, Some(name)));
  try!(write_dir_entry_rec_len(fs, dir_inode, free_space.prev_offset,
    (free_space.offset - free_space.prev_offset) as u16));

  entry_inode.links_count += 1;
  update_inode(fs, entry_inode)
}

fn erase_dir_entry(fs: &mut Filesystem, dir_inode: &mut Inode,
  offset: u64, prev_offset: u64, next_offset: u64) -> Result<()>
{
  let new_entry = DirEntry {
    ino: 0,
    rec_len: (next_offset - offset) as u16,
    name_len: 0,
    file_type: None,
  };

  try!(write_dir_entry(fs, dir_inode, offset, &new_entry, None));

  if offset % fs.block_size() != 0 {
    try!(write_dir_entry_rec_len(fs, dir_inode, prev_offset,
      (next_offset - prev_offset) as u16));
  }
  Ok(())
}

fn read_dir_entry(fs: &mut Filesystem, inode: &Inode, offset: u64) 
  -> Result<(DirEntry, Vec<u8>, u64)>
{
  let mut entry_buffer = make_buffer(dir_entry_size(0));
  try!(read_inode_data(fs, inode, offset, &mut entry_buffer[..]));
  let entry = try!(decode_dir_entry(&fs.superblock, &entry_buffer[..]));

  if entry.rec_len < dir_entry_size(entry.name_len as u64) as u16 {
    return Err(Error::new(format!(
      "Entry at byte {} in directory {} is too short", offset, inode.ino)))
  }

  let mut name_buffer = make_buffer(entry.name_len as u64);
  try!(read_inode_data(fs, &inode, offset + 8, &mut name_buffer[..]));
  Ok((entry, name_buffer, offset + entry.rec_len as u64))
}

fn write_dir_entry(fs: &mut Filesystem, dir_inode: &mut Inode, offset: u64,
  entry: &DirEntry, name: Option<&[u8]>) -> Result<()>
{
  let mut entry_buffer = make_buffer(
    dir_entry_size(name.map(|n| n.len() as u64).unwrap_or(0)));
  try!(encode_dir_entry(&fs.superblock, entry, &mut entry_buffer[..]));
  match name {
    Some(name) =>
      for i in 0..name.len() {
        entry_buffer[dir_entry_size(0) as usize + i] = name[i];
      },
    None => (),
  }
  try!(write_inode_data(fs, dir_inode, offset, &entry_buffer[..]));
  Ok(())
}
  
fn write_dir_entry_rec_len(fs: &mut Filesystem, dir_inode: &mut Inode,
  offset: u64, rec_len: u16) -> Result<()>
{
  let mut minibuf = [0; 2];
  encode_u16(rec_len, &mut minibuf[..]);
  try!(write_inode_data(fs, dir_inode, offset + 4, &minibuf[..]));
  Ok(())
}

fn dir_entry_size(name_len: u64) -> u64 {
  8 + name_len
}

fn space_in_block(fs: &mut Filesystem, offset: u64) -> u64 {
  let block_size = fs.block_size();
  let block_offset = offset / block_size * block_size;
  (block_offset + block_size) - offset
}

fn align_4(x: u64) -> u64 {
  (x + 0b11) & !0b11
}
