#[derive(Debug, Copy, Clone)]
pub struct Superblock {
  pub blocks_count: u32,
  pub free_blocks_count: u32,
  pub free_inodes_count: u32,
  pub first_data_block: u32,
  pub log_block_size: u32,
  pub blocks_per_group: u32,
  pub inodes_per_group: u32,
  pub state: u16,
  pub rev_level: u32,
  pub first_ino: u32,
  pub inode_size: u16,
  pub feature_compat: u32,
  pub feature_incompat: u32,
  pub feature_ro_compat: u32,
}

pub const SUPERBLOCK_MAGIC: u16 = 0xef53;
pub const SUPPORTED_INCOMPAT_FEATURES: u32 = 0x0002;
pub const SUPPORTED_RO_COMPAT_FEATURES: u32 = 0;

#[derive(Debug, Copy, Clone)]
pub struct GroupDesc {
  pub block_bitmap: u32,
  pub inode_bitmap: u32,
  pub inode_table: u32,
  pub free_blocks_count: u16,
  pub free_inodes_count: u16,
  pub used_dirs_count: u16,
}

#[derive(Debug, Copy, Clone)]
pub struct Inode {
  pub ino: u64,
  pub mode: Mode,
  pub attr: FileAttr,
  pub size: u64,
  pub size_512: u32,
  pub links_count: u16,
  pub flags: u32,
  pub block: [u32; 15],
  pub file_acl: u32,
}

#[derive(Debug, Copy, Clone)]
pub struct Mode {
  pub file_type: FileType,
  pub suid: bool,
  pub sgid: bool,
  pub sticky: bool,
  pub access_rights: u16,
}

#[derive(Debug, Copy, Clone)]
pub struct FileAttr {
  pub uid: u32,
  pub gid: u32,
  pub atime: u32,
  pub ctime: u32,
  pub mtime: u32,
  pub dtime: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FileType {
  Regular,
  Dir,
  CharDev,
  BlockDev,
  Fifo,
  Socket,
  Symlink,
}

#[derive(Copy, Clone, Debug)]
pub struct DirEntry {
  pub ino: u32,
  pub rec_len: u16,
  pub name_len: u8,
  pub file_type: Option<FileType>,
}

