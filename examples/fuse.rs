extern crate ext2;
extern crate fuse;
extern crate libc;
extern crate time;

use std::{error, fs, path};
use std::collections::{HashMap};
use std::os::unix::ffi::{OsStrExt};
use std::ffi::{OsStr};

struct Fuse {
  fs: ext2::Filesystem,
  dir_handles: HashMap<u64, ext2::DirHandle>,
  file_handles: HashMap<u64, ext2::FileHandle>,
  next_fh: u64,
}

enum Handle {
  Dir(ext2::DirHandle),
  File(ext2::FileHandle),
}

impl Fuse {
  fn new(fs: ext2::Filesystem) -> Fuse {
    Fuse {
      fs: fs,
      dir_handles: HashMap::new(),
      file_handles: HashMap::new(),
      next_fh: 0,
    }
  }
}

const TTL: time::Timespec = time::Timespec { sec: 1, nsec: 0 };

#[allow(unused_variables)]
impl fuse::Filesystem for Fuse {
  fn lookup(&mut self, _req: &fuse::Request,
    parent_ino: u64, name: &path::Path, reply: fuse::ReplyEntry)
  {
    let res: Result<_, ext2::Error> = (|| {
      let parent_inode = try!(self.fs.read_inode(ext2_ino(parent_ino)));
      let entry = try!(self.fs.dir_lookup(
          parent_inode, name.as_os_str().as_bytes()));
      Ok(match entry {
        Some(entry_ino) => {
          let entry_inode = try!(self.fs.read_inode(entry_ino));
          Some(inode_to_file_attr(entry_ino, &entry_inode))
        },
        None => None,
      })
    })();

    match res {
      Err(_err) => reply.error(65),
      Ok(None) => reply.error(libc::ENOENT),
      Ok(Some(file_attr)) => reply.entry(&TTL, &file_attr, 0),
    }
  }

  fn getattr(&mut self, _req: &fuse::Request, ino: u64, reply: fuse::ReplyAttr) {
    println!("getattr (ino {})", ino);
    match self.fs.read_inode(ext2_ino(ino)) {
      Err(_err) => reply.error(65),
      Ok(inode) => reply.attr(&TTL, &inode_to_file_attr(ext2_ino(ino), &inode)),
    }
  }

  /*
  fn readlink(&mut self, _req: &fuse::Request, ino: u64, reply: fuse::ReplyData) {
    reply.error(65)
  }

  fn open(&mut self, _req: &fuse::Request, ino: u64, flags: u32, reply: fuse::ReplyOpen) {
    reply.error(65)
  }

  fn read(&mut self, _req: &fuse::Request, ino: u64, fh: u64,
    offset: u64, size: u32, reply: fuse::ReplyData)
  {
    reply.error(65)
  }

  fn release(&mut self, _req: &fuse::Request, ino: u64, fh: u64,
    flags: u32, lock_owner: u64, flush: bool, reply: fuse::ReplyEmpty)
  {
    reply.error(65)
  }
  */

  fn opendir(&mut self, _req: &fuse::Request, ino: u64,
    flags: u32, reply: fuse::ReplyOpen)
  {
    let res: Result<_, ext2::Error> = (|| {
      let fh = self.next_fh;
      self.next_fh += 1;
      let dir_inode = try!(self.fs.read_inode(ext2_ino(ino)));
      let dir_handle = try!(self.fs.dir_open(dir_inode));
      self.dir_handles.insert(fh, dir_handle);
      Ok(fh)
    })();

    match res {
      Err(_err) => reply.error(65),
      Ok(fh) => reply.opened(fh, 0),
    }
  }

  fn readdir(&mut self, _req: &fuse::Request, ino: u64, fh: u64,
    offset: u64, mut reply: fuse::ReplyDirectory)
  {
    let res: Result<_, ext2::Error> = (|| {
      let handle = try!(self.dir_handles.get_mut(&fh)
          .ok_or_else(|| ext2::Error::new(format!("Bad dir handle"))));
      while let Some(line) = try!(self.fs.dir_read(handle)) {
        let ino = fuse_ino(line.ino);
        let file_type = fuse_file_type(line.file_type);
        let name = <OsStr as OsStrExt>::from_bytes(&line.name[..]);
        if reply.add(ino, 0, file_type, name) {
          break
        }
      }
      Ok(())
    })();

    match res {
      Err(_err) => reply.error(65),
      Ok(()) => reply.ok(),
    }
  }

  fn releasedir(&mut self, _req: &fuse::Request, _ino: u64, fh: u64,
    _flags: u32, reply: fuse::ReplyEmpty)
  {
    self.dir_handles.remove(&fh);
    reply.ok();
  }
}

fn mein() -> Result<(), ext2::Error> {
  let file = try!(fs::File::open("test.ext2"));
  let reader = ext2::FileReader(file);
  let fs = try!(ext2::Filesystem::new(Box::new(reader)));
  let fuse = Fuse::new(fs);
  fuse::mount(fuse, &"/tmp/test", &[]);
  Ok(())
}

fn main() {
  match mein() {
    Ok(()) => {},
    Err(err) => print_error(&err),
  }
}

fn print_error(err: &error::Error) {
  println!("Error: {}", err);
  match err.cause() {
    Some(cause) => print_error(cause),
    None => (),
  }
}

fn ext2_ino(fuse_ino: u64) -> u64 {
  if fuse_ino == 1 { ext2::Filesystem::ROOT_INO } else { fuse_ino }
}

fn fuse_ino(ext2_ino: u64) -> u64 {
  if ext2_ino == ext2::Filesystem::ROOT_INO { 1 } else { ext2_ino }
}

fn inode_to_file_attr(ext2_ino: u64, inode: &ext2::Inode) -> fuse::FileAttr {
  fuse::FileAttr {
    ino: fuse_ino(ext2_ino),
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

fn fuse_file_type(ftype: ext2::FileType) -> fuse::FileType {
  match ftype {
    ext2::FileType::Regular => fuse::FileType::RegularFile,
    ext2::FileType::Dir => fuse::FileType::Directory,
    ext2::FileType::CharDev => fuse::FileType::CharDevice,
    ext2::FileType::BlockDev => fuse::FileType::BlockDevice,
    ext2::FileType::Fifo => fuse::FileType::NamedPipe,
    ext2::FileType::Socket => panic!("Fuse cannot handle sockets"),
    ext2::FileType::Symlink => fuse::FileType::Symlink,
  }
}

