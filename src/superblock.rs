use error::{Error, Result};
use read_int::{read_u16, read_u32};

#[derive(Debug)]
pub struct Superblock {
  pub first_data_block: u32,
  pub log_block_size: u32,
  pub blocks_per_group: u32,
  pub inodes_per_group: u32,
  pub rev_level: u32,
  pub first_ino: u32,
  pub inode_size: u16,
  pub feature_compat: u32,
  pub feature_incompat: u32,
  pub feature_ro_compat: u32,
}

static MAGIC: u16 = 0xef53;
static FEATURE_COMPAT_DIR_PREALLOC: u32 = 0x0001;
static FEATURE_COMPAT_IMAGIC_INODES: u32 = 0x0002;
static FEATURE_COMPAT_HAS_JOURNAL: u32 = 0x0004;
static FEATURE_COMPAT_EXT_ATTR: u32 = 0x0008;
static FEATURE_COMPAT_RESIZE_INO: u32 = 0x0010;
static FEATURE_COMPAT_DIR_INDEX: u32 = 0x0020;
static FEATURE_INCOMPAT_COMPRESSION: u32 = 0x0001;
static FEATURE_INCOMPAT_FILETYPE: u32 = 0x0002;
static FEATURE_INCOMPAT_RECOVER: u32 = 0x0004;
static FEATURE_INCOMPAT_JOURNAL_DEV: u32 = 0x0008;
static FEATURE_INCOMPAT_META_BG: u32 = 0x0010;
static FEATURE_RO_COMPAT_SPARSE_SUPER: u32 = 0x0001;
static FEATURE_RO_COMPAT_LARGE_FILE: u32 = 0x0002;
static FEATURE_RO_COMPAT_BTREE_DIR: u32 = 0x0004;

static SUPPORTED_INCOMPAT: u32 = 0x0002;
static SUPPORTED_RO_COMPAT: u32 = 0;

impl Superblock {
  pub fn decode(bytes: &[u8], read_only: bool) -> Result<Superblock> {
    assert!(bytes.len() >= 1024);
    let magic = read_u16(&bytes[56..]);
    let state = read_u16(&bytes[58..]);
    let rev = read_u32(&bytes[76..]);
    let feature_compat = if rev >= 1 { read_u32(&bytes[92..]) } else { 0 };
    let feature_incompat = if rev >= 1 { read_u32(&bytes[96..]) } else { 0 };
    let feature_ro_compat = if rev >= 1 { read_u32(&bytes[100..]) } else { 0 };

    if magic != MAGIC {
      return Err(Error::new(format!("Bad magic 0x{:x}, expected 0x{:x}", magic, MAGIC)));
    }

    if state != 1 {
      return Err(Error::new(format!("Volume is in an invalid state (0x{:x})", state)));
    }

    if (feature_incompat & !SUPPORTED_INCOMPAT) != 0 {
      return Err(Error::new(format!("Volume uses incompatible features (0x{:x})",
              feature_incompat)));
    }

    if !read_only && (feature_ro_compat & !SUPPORTED_RO_COMPAT) != 0 {
      return Err(Error::new(format!(
              "Volume uses incompatible features, only reading is possible (0x{:x})",
              feature_ro_compat)));
    }

    Ok(Superblock {
      first_data_block: read_u32(&bytes[20..]),
      log_block_size: read_u32(&bytes[24..]),
      blocks_per_group: read_u32(&bytes[32..]),
      inodes_per_group: read_u32(&bytes[40..]),
      rev_level: rev,
      first_ino: if rev >= 1 { read_u32(&bytes[84..]) } else { 11 },
      inode_size: if rev >= 1 { read_u16(&bytes[88..]) } else { 128 },
      feature_compat: feature_compat,
      feature_incompat: feature_incompat,
      feature_ro_compat: feature_ro_compat,
    })
  }
}
