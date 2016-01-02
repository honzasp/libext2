use prelude::*;

pub fn make_inode_in_dir(fs: &mut Filesystem, dir_ino: u64,
  name: &[u8], mode: Mode, attr: FileAttr) -> Result<Inode>
{
  let mut dir_inode = try!(get_inode(fs, dir_ino));
  if dir_inode.mode.file_type != FileType::Dir {
    return Err(Error::new(format!(
      "Inode {} is not a directory", dir_ino)));
  }

  let dir_group = get_ino_group(fs, dir_ino).0;
  let new_ino = match try!(alloc_inode(fs, dir_group)) {
    None => return Err(Error::new(format!("No free inodes left"))),
    Some(ino) => ino,
  };

  let mut new_inode = try!(init_inode(fs, &mut dir_inode, new_ino, mode, attr));
  try!(add_dir_entry(fs, &mut dir_inode, &mut new_inode, name));
  Ok(new_inode)
}

pub fn make_symlink_in_dir(fs: &mut Filesystem, dir_ino: u64,
  name: &[u8], link: &[u8], attr: FileAttr) -> Result<Inode>
{
  let mode = Mode {
    file_type: FileType::Symlink,
    suid: false, sgid: false, sticky: false,
    access_rights: 0o777,
  };
  let mut inode = try!(make_inode_in_dir(fs, dir_ino, name, mode, attr));
  try!(write_link_data(fs, &mut inode, link));
  Ok(inode)
}

pub fn make_hardlink_in_dir(fs: &mut Filesystem, dir_ino: u64,
  name: &[u8], link_ino: u64) -> Result<Inode>
{
  let mut dir_inode = try!(get_inode(fs, dir_ino));
  let mut link_inode = try!(get_inode(fs, link_ino));

  if dir_inode.mode.file_type != FileType::Dir {
    return Err(Error::new(format!("Inode {} is not a directory", dir_ino)));
  } else if link_inode.mode.file_type == FileType::Dir {
    return Err(Error::new(format!("Inode {} is a directory", link_ino)));
  }

  try!(add_dir_entry(fs, &mut dir_inode, &mut link_inode, name));
  Ok(link_inode)
}

