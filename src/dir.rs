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
  let mut handle = try!(open_dir(fs, dir_ino));
  while let Some(line) = try!(read_dir(fs, &mut handle)) {
    if line.name == name {
      return Ok(Some(line.ino));
    }
  }
  Ok(None)
}

pub fn open_dir(fs: &mut Filesystem, ino: u64) -> Result<DirHandle> {
  let inode = try!(get_inode(fs, ino));
  if inode.file_type == FileType::Dir {
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

  let mut entry_buffer = make_buffer(8);
  try!(read_inode_data(fs, &inode, handle.offset, &mut entry_buffer[..]));
  let entry = try!(decode_dir_entry(&fs.superblock, &entry_buffer[..]));

  let mut name_buffer = make_buffer(entry.name_len as u64);
  try!(read_inode_data(fs, &inode, handle.offset + 8, &mut name_buffer[..]));

  let file_type = match entry.file_type {
    Some(file_type) => file_type,
    None => try!(get_inode(fs, entry.ino as u64)).file_type,
  };

  handle.offset = handle.offset + entry.rec_len as u64;
  Ok(Some(DirLine {
    ino: entry.ino as u64,
    file_type: file_type,
    name: name_buffer,
  }))
}

pub fn close_dir(_fs: &mut Filesystem, _handle: DirHandle) -> Result<()> {
  Ok(())
}

