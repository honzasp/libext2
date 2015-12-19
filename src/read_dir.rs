use std::{iter};
use error::{Result, Error};
use dir_entry::{DirEntry};
use inode::{Inode, FileType};
use context::{Context};
use read_file;

pub struct Cursor<'i> {
  inode: &'i Inode,
  pos: u64,
  block_buf: Option<Vec<u8>>,
}

pub fn init_cursor<'i>(_ctx: &Context, inode: &'i Inode) -> Result<Cursor<'i>> {
  if inode.file_type == FileType::Dir {
    Ok(Cursor { inode: inode, pos: 0, block_buf: None })
  } else {
    Err(Error::new(format!("Inode is not a directory but {:?}", inode.file_type)))
  }
}

pub fn advance_cursor<'i>(ctx: &Context, cursor: &mut Cursor<'i>)
  -> Result<Option<DirEntry>>
{
  if cursor.pos >= cursor.inode.size {
    return Ok(None)
  }

  let block_idx = cursor.pos / ctx.block_size();
  let block_pos = cursor.pos % ctx.block_size();

  if cursor.block_buf.is_none() {
    let mut buf: Vec<u8> = iter::repeat(0).take(ctx.block_size() as usize).collect();
    try!(read_file::read_block(ctx, cursor.inode, block_idx, 0, &mut buf[..]));
    cursor.block_buf = Some(buf);
  }

  let entry = try!(DirEntry::decode(ctx.superblock(),
      &cursor.block_buf.as_ref().unwrap()[block_pos as usize..]));

  cursor.pos += entry.rec_len as u64;
  if cursor.pos / ctx.block_size() != block_idx {
    cursor.block_buf = None;
  }

  Ok(Some(entry))
}
