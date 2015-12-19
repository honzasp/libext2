#[derive(Debug)]
pub struct Inode {
  pub mode: u16,
  pub uid: u16,
  pub gid: u16,
  pub size: u64,
  pub atime: u32,
  pub ctime: u32,
  pub mtime: u32,
  pub dtime: u32,
  pub links_count: u16,
  pub blocks_512: u32,
  pub flags: u32,
  pub block: [u32; 15],
}

#[derive(Copy, Clone, Debug)]
pub enum FileType {
  Unknown = 0,
  Regular = 1,
  Dir = 2,
  CharDev = 3,
  BlockDev = 4,
  Fifo = 5,
  Socket = 6,
  Symlink = 7,
}

