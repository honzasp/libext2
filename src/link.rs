use std::{cmp};
use prelude::*;

pub fn read_link(fs: &mut Filesystem, ino: u64) -> Result<Vec<u8>> {
  let inode = try!(get_inode(fs, ino));
  if inode.mode.file_type == FileType::Symlink {
    read_link_data(fs, &inode)
  } else {
    Err(Error::new(format!("inode is not a symlink")))
  }
}

pub fn is_fast_symlink(fs: &Filesystem, inode: &Inode) -> bool {
  if inode.mode.file_type != FileType::Symlink {
    return false
  }

  if inode.file_acl != 0 {
    inode.size_512 as u64 == fs.block_size() / 512 
  } else {
    inode.size_512 == 0
  }
}

pub fn read_link_data(fs: &mut Filesystem, inode: &Inode) -> Result<Vec<u8>> {
  let mut buffer = make_buffer(inode.size + 4);

  let length = 
    if is_fast_symlink(fs, &inode) {
      for i in 0..cmp::min(inode.block.len(), inode.size as usize / 4 + 1) {
        encode_u32(inode.block[i], &mut buffer[4*i..]);
      }
      inode.size
    } else {
      try!(read_inode_data(fs, &inode, 0, &mut buffer[..]))
    };
  buffer.truncate(length as usize);
  Ok(buffer)
}

pub fn write_link_data(fs: &mut Filesystem, inode: &mut Inode, data: &[u8]) -> Result<()> {
  try!(truncate_inode_blocks(fs, inode, 0));
  if data.len() <= 15 * 4 {
    use std::iter;
    let data_buf: Vec<u8> = data.iter().cloned().chain(iter::repeat(0))
      .take(15 * 4).collect();
    for i in 0..15 {
      inode.block[i] = decode_u32(&data_buf[i*4..]);
    }
    inode.size = data.len() as u64;
    try!(update_inode(fs, inode));
  } else {
    try!(write_inode_data(fs, inode, 0, data));
  }
  Ok(())
}
