use std::{cmp};
use prelude::*;

pub fn read_link(fs: &mut Filesystem, ino: u64) -> Result<Vec<u8>> {
  let inode = try!(get_inode(fs, ino));
  if inode.mode.file_type == FileType::Symlink {
    let fast_symlink =
      if inode.file_acl != 0 {
        inode.size_512 as u64 == fs.block_size() / 512 
      } else {
        inode.size_512 == 0
      };
    let mut buffer = make_buffer(inode.size + 4);

    let length = 
      if fast_symlink {
        for i in 0..cmp::min(inode.block.len(), inode.size as usize / 4 + 1) {
          encode_u32(inode.block[i], &mut buffer[4*i..]);
        }
        inode.size
      } else {
        try!(read_inode_data(fs, &inode, 0, &mut buffer[..]))
      };
    buffer.truncate(length as usize);
    Ok(buffer)
  } else {
    Err(Error::new(format!("inode is not a symlink")))
  }
}

