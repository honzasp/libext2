use prelude::*;

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

pub fn lookup_dir(fs: &mut Filesystem, dir_inode: Inode, name: &[u8]) 
  -> Result<Option<u64>> 
{
  let mut handle = try!(open_dir(fs, dir_inode));
  while let Some(line) = try!(read_dir(fs, &mut handle)) {
    if line.name == name {
      return Ok(Some(line.ino));
    }
  }
  Ok(None)
}

pub fn open_dir(_fs: &mut Filesystem, inode: Inode) -> Result<DirHandle> {
  if inode.file_type == FileType::Dir {
    Ok(DirHandle { inode: inode, offset: 0, cache: None })
  } else {
    return Err(Error::new(format!("inode is not a directory")))
  }
}

pub fn read_dir(fs: &mut Filesystem, handle: &mut DirHandle) 
  -> Result<Option<DirLine>> 
{
  if handle.offset >= handle.inode.size {
    return Ok(None)
  }
  let block_idx = handle.offset / fs.block_size();
  let block_pos = handle.offset % fs.block_size();

  let cache_valid = if let Some((cached_idx, _)) = handle.cache {
      cached_idx == block_idx
    } else {
      false
    };

  if !cache_valid {
    let mut buffer = make_buffer(fs.block_size());
    try!(read_inode_block(fs, &handle.inode, block_idx, 0, &mut buffer[..]));
    handle.cache = Some((block_idx, buffer));
  }

  let block = &handle.cache.as_ref().unwrap().1;
  let entry = try!(decode_dir_entry(
      &fs.superblock, &block[block_pos as usize..]));
  let file_type = match entry.file_type {
    Some(file_type) => file_type,
    None => try!(read_inode(fs, entry.ino as u64)).file_type,
  };

  handle.offset = handle.offset + entry.rec_len as u64;
  Ok(Some(DirLine {
    ino: entry.ino as u64,
    file_type: file_type,
    name: entry.name 
  }))
}

pub fn close_dir(_fs: &mut Filesystem, _handle: DirHandle) -> Result<()> {
  Ok(())
}

