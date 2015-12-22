/*
use libc;
use fuse;
use time;
use std::{path};
use std::os::unix::ffi::{OsStrExt};
use std::ffi::{OsStr};
use context::{Context};
use inode::{Inode, FileType};
use read_dir;
use error::{Result};

pub struct Fuse {
  ctx: Context,
  dir_cursors: Vec<Option<read_dir::Cursor>>,
}

impl Fuse {
  pub fn new(ctx: Context) -> Fuse {
    Fuse { ctx: ctx, dir_cursors: Vec::new() }
  }
}

const TTL: time::Timespec = time::Timespec { sec: 1, nsec: 0 };

#[allow(unused_variables)]
impl fuse::Filesystem for Fuse {
  fn lookup(&mut self, _req: &fuse::Request,
    parent_ino: u64, name: &path::Path, reply: fuse::ReplyEntry)
  {
    println!("lookup (parent {}, name {:?})", parent_ino, name);

    let ino_inode: Result<_> = (|| {
      let parent_inode = try!(self.ctx.read_inode(parent_ino));
      let name_bytes = name.as_os_str().as_bytes();
      Ok(if let Some(entry) =
        try!(read_dir::seek_entry(&self.ctx, parent_inode, name_bytes))
      {
        Some((entry.ino as u64, try!(self.ctx.read_inode(entry.ino as u64))))
      } else {
        None
      })
    })();

    match ino_inode {
      Err(_err) => reply.error(123),
      Ok(None) => reply.error(libc::ENOENT),
      Ok(Some((ino, inode))) =>
        reply.entry(&TTL, &inode_to_file_attr(ino, &inode), 0),
    }
  }

  fn getattr(&mut self, _req: &fuse::Request, ino: u64, reply: fuse::ReplyAttr) {
    println!("getattr (ino {})", ino);
    match self.ctx.read_inode(ino) {
      Err(_err) => { println!("{:?}", _err); reply.error(123); },
      Ok(inode) => reply.attr(&TTL, &inode_to_file_attr(ino, &inode)),
    }
  }

  fn readlink(&mut self, _req: &fuse::Request, ino: u64, reply: fuse::ReplyData) {
    reply.error(123)
  }

  fn open(&mut self, _req: &fuse::Request, ino: u64, flags: u32, reply: fuse::ReplyOpen) {
    // TODO: "open" a file. the reply can specify a 64-bit file handle (fh) that
    // will be given back to us in read()/write(). this can be used as an index
    // or something.
    reply.error(123)
  }

  fn read(&mut self, _req: &fuse::Request, ino: u64, fh: u64,
    offset: u64, size: u32, reply: fuse::ReplyData)
  {
    // should read exactly the number of bytes requested and send it to reply
    reply.error(123)
  }

  fn release(&mut self, _req: &fuse::Request, ino: u64, fh: u64,
    flags: u32, lock_owner: u64, flush: bool, reply: fuse::ReplyEmpty)
  {
    // should drop the context initialized in open()
    reply.error(123)
  }

  fn opendir(&mut self, _req: &fuse::Request, ino: u64,
    flags: u32, reply: fuse::ReplyOpen)
  {
    println!("opendir (ino {})", ino);

    let cursor = (|| {
      let inode = try!(self.ctx.read_inode(ino));
      read_dir::init_cursor(&self.ctx, inode)
    })();

    match cursor {
      Err(_err) => reply.error(123),
      Ok(cursor) => {
        for handle in 0..self.dir_cursors.len() {
          if self.dir_cursors[handle].is_none() {
            self.dir_cursors[handle] = Some(cursor);
            return reply.opened(handle as u64, 0);
          }
        }
        self.dir_cursors.push(Some(cursor));
        reply.opened(self.dir_cursors.len() as u64 - 1, 0)
      }
    }
  }

  fn readdir(&mut self, _req: &fuse::Request, ino: u64, fh: u64,
    offset: u64, mut reply: fuse::ReplyDirectory)
  {
    println!("readdir (ino {}, fh {}, offset {})", ino, fh, offset);

    let res: Result<()> = (|| {
      let cursor = self.dir_cursors[fh as usize].as_mut().unwrap();
      //if offset != 0 {
        //read_dir::set_cursor_hint(&self.ctx, cursor, offset);
      //}

      let inode = try!(self.ctx.read_inode(ino));

      while let Some(entry) = try!(read_dir::advance_cursor(&self.ctx, cursor)) {
        if entry.ino == 0 {
          continue
        }

        let kind = fuse_file_type(match entry.file_type {
          Some(ftype) => ftype,
          None => try!(self.ctx.read_inode(entry.ino as u64)).file_type,
        });
        let name = OsStr::from_bytes(&entry.name[..]);
        let offset = read_dir::get_cursor_hint(&self.ctx, cursor);

        if reply.add(entry.ino as u64, offset, kind, name) {
          break
        }
      }
      Ok(())
    })();

    match res {
      Err(_err) => reply.error(123),
      Ok(()) => reply.ok(),
    }
  }

  fn releasedir(&mut self, _req: &fuse::Request, _ino: u64, fh: u64,
    _flags: u32, reply: fuse::ReplyEmpty)
  {
    println!("releasedir (ino {}, fh {})", _ino, fh);
    self.dir_cursors[fh as usize] = None;
    reply.ok();
  }
}

fn inode_to_file_attr(ino: u64, inode: &Inode) -> fuse::FileAttr {
  fuse::FileAttr {
    ino: ino,
    size: inode.size,
    blocks: inode.size_512 as u64,
    atime: fuse_timespec(inode.atime),
    ctime: fuse_timespec(inode.ctime),
    mtime: fuse_timespec(inode.mtime),
    crtime: fuse_timespec(0),
    kind: fuse_file_type(inode.file_type),
    perm: inode.access_rights.0,
    nlink: inode.links_count as u32,
    uid: inode.uid,
    gid: inode.gid,
    rdev: 0,
    flags: 0,
  }
}

fn fuse_timespec(epoch: u32) -> time::Timespec {
  time::Timespec::new(epoch as i64, 0)
}

fn fuse_file_type(ftype: FileType) -> fuse::FileType {
  match ftype {
    FileType::Regular => fuse::FileType::RegularFile,
    FileType::Dir => fuse::FileType::Directory,
    FileType::CharDev => fuse::FileType::CharDevice,
    FileType::BlockDev => fuse::FileType::BlockDevice,
    FileType::Fifo => fuse::FileType::NamedPipe,
    FileType::Socket => panic!("Fuse cannot handle sockets"),
    FileType::Symlink => fuse::FileType::Symlink,
  }
}
*/
