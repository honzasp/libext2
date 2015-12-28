use std::{cmp};
use prelude::*;

#[derive(Debug)]
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

pub fn lookup_dir(fs: &mut Filesystem, dir_ino: u64, name: &[u8]) 
  -> Result<Option<u64>> 
{
  let inode = try!(get_inode(fs, dir_ino));
  if inode.mode.file_type != FileType::Dir {
    return Err(Error::new(format!("inode {} is not a directory", dir_ino)))
  }

  let mut offset = 0;
  while offset < inode.size {
    let (entry, entry_name, next_offset) = try!(read_dir_entry(fs, &inode, offset));
    if entry.ino != 0 && name == &entry_name[..] {
      return Ok(Some(entry.ino as u64))
    }
    offset = next_offset;
  }
  Ok(None)
}

pub fn open_dir(fs: &mut Filesystem, ino: u64) -> Result<DirHandle> {
  let inode = try!(get_inode(fs, ino));
  if inode.mode.file_type == FileType::Dir {
    Ok(DirHandle { ino: ino, offset: 0 })
  } else {
    return Err(Error::new(format!("inode is not a directory")))
  }
}

pub fn read_dir(fs: &mut Filesystem, handle: &mut DirHandle) 
  -> Result<Option<DirLine>> 
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
      return Ok(Some(DirLine {
        ino: entry.ino as u64,
        file_type: file_type,
        name: name,
      }))
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
  println!("add_dir_entry (size {})", entry_size);

  let mut place_for_entry = None;
  let mut offset = 0;
  let mut last_offset = 0;
  while offset < dir_inode.size {
    let (entry, entry_name, next_offset) = try!(read_dir_entry(fs, dir_inode, offset));
    println!("{2} => {3}: entry {0:?} {1:?}", entry, entry_name, offset, next_offset);

    if entry_name == name {
      let new_entry = DirEntry {
        ino: entry_inode.ino as u32,
        rec_len: entry.rec_len,
        name_len: entry.name_len,
        file_type: Some(entry_inode.mode.file_type),
      };
      return write_dir_entry(fs, dir_inode, offset, &new_entry, None);
    }

    let free_offset =
      align_4(if entry.ino == 0 {
        offset
      } else {
        offset + dir_entry_size(entry_name.len() as u64)
      });
    let free_size = cmp::min(next_offset - free_offset, space_in_block(fs, offset));
    println!("  free {} at {}", free_size, free_offset);

    if place_for_entry.is_none() && free_size >= entry_size {
      place_for_entry = Some(FreeSpace {
        offset: free_offset,
        prev_offset: offset,
        next_offset: next_offset,
      });
      println!("  place for entry {:?}", place_for_entry);
    }

    last_offset = offset;
    offset = next_offset;
  }

  insert_dir_entry(fs, dir_inode, entry_inode, name, place_for_entry, last_offset)
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

fn write_dir_entry(fs: &mut Filesystem, inode: &mut Inode, offset: u64,
  entry: &DirEntry, name: Option<&[u8]>) -> Result<()>
{
  println!("write_dir_entry(ino {}, offset {}, {:?}, {:?})",
    inode.ino, offset, entry, name);
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
  try!(write_inode_data(fs, inode, offset, &entry_buffer[..]));
  Ok(())
}
  
fn write_dir_entry_rec_len(fs: &mut Filesystem, inode: &mut Inode,
  offset: u64, rec_len: u16) -> Result<()>
{
  println!("write_dir_entry_rec_len(ino {}, offset {}, {})",
    inode.ino, offset, rec_len);
  let mut minibuf = [0; 2];
  encode_u16(rec_len, &mut minibuf[..]);
  try!(write_inode_data(fs, inode, offset + 4, &minibuf[..]));
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
