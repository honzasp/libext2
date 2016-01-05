use std::{iter};
use std::collections::{HashMap, HashSet, VecDeque};
use prelude::*;

pub struct Filesystem {
  pub volume: Box<Volume>,
  pub superblock: Superblock,
  pub superblock_bytes: Vec<u8>,
  pub superblock_dirty: bool,
  pub groups: Vec<Group>,
  pub inode_cache: HashMap<u64, Inode>,
  pub dirty_inos: HashSet<u64>,
  pub reused_inos: HashSet<u64>,
  pub cache_queue: VecDeque<u64>,
}

pub struct Group {
  pub idx: u64,
  pub desc: GroupDesc,
  pub block_bitmap: Vec<u8>,
  pub inode_bitmap: Vec<u8>,
  pub dirty: bool,
}

pub const ROOT_INO: u64 = 2;

impl Filesystem {
  pub fn block_size(&self) -> u64 {
    1024 << self.superblock.log_block_size 
  }

  pub fn group_count(&self) -> u64 {
    let a = self.superblock.blocks_count as u64;
    let b = self.superblock.blocks_per_group as u64;
    (a + b - 1) / b
  }
}

pub fn mount_fs(mut volume: Box<Volume>) -> Result<Filesystem> {
  let mut superblock_bytes = make_buffer(1024);
  try!(volume.read(1024, &mut superblock_bytes[..]));
  let superblock = try!(decode_superblock(&superblock_bytes[..], true));

  let mut fs = Filesystem {
    volume: volume,
    superblock: superblock,
    superblock_bytes: superblock_bytes,
    superblock_dirty: false,
    groups: Vec::new(),
    inode_cache: HashMap::new(),
    dirty_inos: HashSet::new(),
    reused_inos: HashSet::new(),
    cache_queue: VecDeque::new(),
  };

  for group_idx in 0..fs.group_count() {
    let group = try!(read_group(&mut fs, group_idx));
    fs.groups.push(group);
  }

  try!(flush_superblock(&mut fs, false));
  Ok(fs)
}

pub fn flush_fs(fs: &mut Filesystem) -> Result<()> {
  let dirty_inos = fs.dirty_inos.clone();
  for dirty_ino in dirty_inos {
    try!(flush_ino(fs, dirty_ino));
  }

  for group_idx in 0..fs.group_count() {
    try!(flush_group(fs, group_idx));
  }

  flush_superblock(fs, true)
}

fn flush_superblock(fs: &mut Filesystem, clean: bool) -> Result<()> {
  let state = if clean { 1 } else { 2 };
  fs.superblock_dirty = fs.superblock_dirty || fs.superblock.state != state;
  fs.superblock.state = state;

  if fs.superblock_dirty {
    try!(encode_superblock(&fs.superblock, &mut fs.superblock_bytes[..]));
    try!(fs.volume.write(1024, &fs.superblock_bytes[..]));
    fs.superblock_dirty = false;
  }
  Ok(())
}

pub fn make_buffer(size: u64) -> Vec<u8> {
  iter::repeat(0).take(size as usize).collect()
}
