use prelude::*;

pub fn get_inode(fs: &mut Filesystem, ino: u64) -> Result<Inode> {
  if let Some(inode) = fs.inode_cache.get(&ino) {
    fs.reused_inos.insert(ino);
    return Ok(inode.clone())
  }

  let inode = try!(read_inode(fs, ino));
  fs.inode_cache.insert(ino, inode.clone());
  fs.cache_queue.push_back(ino);
  try!(refit_inode_cache(fs));
  Ok(inode)
}

pub fn update_inode(fs: &mut Filesystem, inode: &Inode) -> Result<()> {
  use std::collections::hash_map::Entry;

  fs.dirty_inos.insert(inode.ino);
  match fs.inode_cache.entry(inode.ino) {
    Entry::Occupied(mut occupied) => {
      occupied.insert(inode.clone());
      fs.reused_inos.insert(inode.ino);
      return Ok(())
    },
    Entry::Vacant(vacant) => {
      vacant.insert(inode.clone());
      fs.cache_queue.push_back(inode.ino);
    },
  }

  refit_inode_cache(fs)
}

fn refit_inode_cache(fs: &mut Filesystem) -> Result<()> {
  while fs.inode_cache.len() > 10 {
    let mut flushed = false;

    while let Some(used_ino) = fs.cache_queue.pop_front() {
      if fs.reused_inos.remove(&used_ino) {
        try!(flush_ino(fs, used_ino));
        flushed = true;
        break;
      }
    }

    if !flushed {
      let random_ino = *fs.inode_cache.iter().next().unwrap().0;
      try!(flush_ino(fs, random_ino));
    }
  }
  Ok(())
}

pub fn flush_ino(fs: &mut Filesystem, ino: u64) -> Result<()> {
  if let Some(inode) = fs.inode_cache.remove(&ino) {
    fs.reused_inos.remove(&ino);
    if fs.dirty_inos.remove(&ino) {
      return write_inode(fs, &inode);
    }
  }
  Ok(())
}


pub fn set_inode_mode_attr(fs: &mut Filesystem, ino: u64,
  mode: Mode, attr: FileAttr) -> Result<()>
{
  let mut inode = try!(get_inode(fs, ino));
  inode.mode = mode;
  inode.attr = attr;
  update_inode(fs, &mut inode)
}

pub fn truncate_inode_size(fs: &mut Filesystem, ino: u64, new_size: u64) -> Result<()> {
  let mut inode = try!(get_inode(fs, ino));
  if inode.mode.file_type != FileType::Regular {
    return Err(Error::new(format!(
      "Cannot truncate inode {} of type {:?}", ino, inode.mode.file_type)));
  }

  if new_size == 0 {
    dealloc_inode_blocks(fs, &mut inode)
  } else if inode.size < new_size {
    return Err(Error::new(format!(
      "Cannot truncate inode {} with size {} to size {}", ino, inode.size, new_size)));
  } else {
    let first_unused_block = (new_size + fs.block_size() - 1) / fs.block_size();
    try!(truncate_inode_blocks(fs, &mut inode, first_unused_block));
    inode.size = new_size;
    update_inode(fs, &mut inode)
  }
}

pub fn inode_mode_from_linux_mode(mode: u16) -> Result<Mode> {
  decode_inode_mode(mode)
}

pub fn unlink_inode(fs: &mut Filesystem, inode: &mut Inode) -> Result<()> {
  if inode.mode.file_type == FileType::Dir {
    if !try!(is_dir_empty(fs, inode)) {
      return Err(Error::new(
          format!("Cannot unlink non-empty directory inode {}", inode.ino)));
    }

    if inode.links_count != 2 {
      return Err(Error::new(format!(
            "Empty directory {} should have 2 links, but has {}",
            inode.ino, inode.links_count)));
    }
    try!(deinit_dir(fs, inode));
  }

  inode.links_count -= 1;
  if inode.links_count == 0 {
    try!(remove_inode(fs, inode))
  }
  update_inode(fs, inode)
}

pub fn read_inode(fs: &mut Filesystem, ino: u64) -> Result<Inode> {
  let (offset, inode_size) = try!(locate_inode(fs, ino));
  let mut inode_buf = make_buffer(inode_size);
  try!(fs.volume.read(offset, &mut inode_buf[..]));
  decode_inode(&fs.superblock, ino, &inode_buf[..])
}

pub fn write_inode(fs: &mut Filesystem, inode: &Inode) -> Result<()> {
  let (offset, inode_size) = try!(locate_inode(fs, inode.ino));
  let mut inode_buf = make_buffer(inode_size);
  try!(fs.volume.read(offset, &mut inode_buf[..]));
  try!(encode_inode(&fs.superblock, inode, &mut inode_buf[..]));
  fs.volume.write(offset, &inode_buf[..])
}

fn locate_inode(fs: &mut Filesystem, ino: u64) -> Result<(u64, u64)> {
  let (group_idx, local_idx) = get_ino_group(fs, ino);
  let inode_size = fs.superblock.inode_size as u64;
  let inode_table = fs.groups[group_idx as usize].desc.inode_table as u64;
  let offset = inode_table * fs.block_size() + local_idx * inode_size;
  Ok((offset, inode_size))
}

pub fn init_inode(fs: &mut Filesystem, dir_inode: &mut Inode,
  ino: u64, mode: Mode, attr: FileAttr) -> Result<Inode> 
{
  let mut inode = Inode {
    ino: ino,
    mode: mode,
    attr: attr,
    size: 0, size_512: 0,
    links_count: 0, flags: 0,
    block: [0; 15],
    file_acl: 0,
  };

  if mode.file_type == FileType::Dir {
    try!(init_dir(fs, dir_inode, &mut inode));
  }
  try!(update_inode(fs, &inode));
  Ok(inode)
}

fn remove_inode(fs: &mut Filesystem, inode: &mut Inode) -> Result<()> {
  try!(dealloc_inode_blocks(fs, inode));
  inode.attr.dtime = 1451303454;
  dealloc_inode(fs, inode.ino)
}
