use prelude::*;

#[derive(Debug)]
pub struct FileHandle {
  inode: Inode,
}

pub fn open_file(_fs: &mut Filesystem, inode: Inode) -> Result<FileHandle> {
  if inode.file_type == FileType::Regular {
    Ok(FileHandle { inode: inode })
  } else {
    Err(Error::new(format!("inode is not a regular file")))
  }
}

pub fn read_file(fs: &mut Filesystem, handle: &mut FileHandle,
    offset: u64, buffer: &mut [u8]) -> Result<u64> 
{
  read_inode_data(fs, &handle.inode, offset, buffer)
}

pub fn write_file(fs: &mut Filesystem, handle: &mut FileHandle,
    offset: u64, buffer: &[u8]) -> Result<u64>
{
  write_inode_data(fs, &mut handle.inode, offset, buffer)
}

pub fn close_file(_fs: &mut Filesystem, _handle: FileHandle) -> Result<()> {
  Ok(())
}
