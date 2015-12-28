use prelude::*;

#[derive(Debug)]
pub struct FileHandle {
  ino: u64,
}

pub fn open_file(fs: &mut Filesystem, ino: u64) -> Result<FileHandle> {
  let inode = try!(get_inode(fs, ino));
  if inode.mode.file_type == FileType::Regular {
    Ok(FileHandle { ino: ino })
  } else {
    Err(Error::new(format!("inode is not a regular file")))
  }
}

pub fn read_file(fs: &mut Filesystem, handle: &mut FileHandle,
    offset: u64, buffer: &mut [u8]) -> Result<u64> 
{
  let inode = try!(get_inode(fs, handle.ino));
  read_inode_data(fs, &inode, offset, buffer)
}

pub fn write_file(fs: &mut Filesystem, handle: &mut FileHandle,
    offset: u64, buffer: &[u8]) -> Result<u64>
{
  let mut inode = try!(get_inode(fs, handle.ino));
  write_inode_data(fs, &mut inode, offset, buffer)
}

pub fn close_file(fs: &mut Filesystem, handle: FileHandle) -> Result<()> {
  flush_ino(fs, handle.ino)
}
