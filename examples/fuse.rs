extern crate ext2;
extern crate fuse;
extern crate libc;
extern crate time;

use std::{error, fs, iter, path};
use std::collections::{HashMap};
use std::os::unix::ffi::{OsStrExt};
use std::ffi::{OsStr};

struct Fuse {
  fs: ext2::Filesystem,
  dir_handles: HashMap<u64, ext2::DirHandle>,
  file_handles: HashMap<u64, ext2::FileHandle>,
  next_fh: u64,
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

impl Drop for Fuse {
  fn drop(&mut self) {
    let _ = ext2::flush_fs(&mut self.fs);
  }
}

impl fuse::Filesystem for Fuse {
  fn destroy(&mut self, _req: &fuse::Request) {
    println!("destroy");
    let _ = ext2::flush_fs(&mut self.fs);
  }

  fn lookup(&mut self, _req: &fuse::Request,
    parent_ino: u64, name: &path::Path, reply: fuse::ReplyEntry)
  {
    println!("lookup (ino {}, name {:?})", parent_ino, 
             &name.to_string_lossy());
    let res: Result<_, ext2::Error> = (|| {
      let entry = try!(ext2::lookup_in_dir(&mut self.fs,
          ext2_ino(parent_ino), name.as_os_str().as_bytes()));
      Ok(match entry {
        Some(entry_ino) => {
          let entry_inode = try!(ext2::get_inode(&mut self.fs, entry_ino));
          Some(inode_to_file_attr(&entry_inode))
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
    match ext2::get_inode(&mut self.fs, ext2_ino(ino)) {
      Err(_err) => reply.error(65),
      Ok(inode) => reply.attr(&TTL, &inode_to_file_attr(&inode)),
    }
  }

  fn readlink(&mut self, _req: &fuse::Request, ino: u64, reply: fuse::ReplyData) {
    println!("readlink (ino {})", ino);
    match ext2::read_link(&mut self.fs, ext2_ino(ino)) {
      Err(_err) => reply.error(65),
      Ok(path) => reply.data(&path[..]),
    }
  }

  fn mknod(&mut self, _req: &fuse::Request, parent: u64, name: &path::Path,
    mode: u32, _rdev: u32, reply: fuse::ReplyEntry)
  {
    println!("mknod (ino {}, name {:?}, mode {:x})", parent, name, mode);
    let res: Result<_, ext2::Error> = (|| {
      ext2::make_inode_in_dir(&mut self.fs, ext2_ino(parent),
        name.as_os_str().as_bytes(), try!(ext2_mode(mode as u16)))
    })();
    match res {
      Err(_err) => reply.error(65),
      Ok(inode) => reply.entry(&TTL, &inode_to_file_attr(&inode), 0),
    }
  }

  fn mkdir(&mut self, req: &fuse::Request, parent: u64, name: &path::Path,
    mode: u32, reply: fuse::ReplyEntry)
  {
    self.mknod(req, parent, name, 0x4000 + (mode & 0xfff), 0, reply)
  }

  fn unlink(&mut self, _req: &fuse::Request, parent: u64,
    name: &path::Path, reply: fuse::ReplyEmpty)
  {
    println!("unlink (ino {}, name {:?})", parent, name);
    match ext2::remove_from_dir(&mut self.fs,
      ext2_ino(parent), name.as_os_str().as_bytes()) 
    {
      Err(_err) => { print_error(&_err); reply.error(65) },
      Ok(true) => reply.ok(),
      Ok(false) => reply.error(libc::ENOENT),
    }
  }

  fn rmdir(&mut self, req: &fuse::Request, parent: u64,
    name: &path::Path, reply: fuse::ReplyEmpty)
  {
    self.unlink(req, parent, name, reply)
  }

  fn open(&mut self, _req: &fuse::Request, ino: u64,
    _flags: u32, reply: fuse::ReplyOpen) 
  {
    println!("open (ino {})", ino);
    match ext2::open_file(&mut self.fs, ext2_ino(ino)) {
      Err(_err) => reply.error(65),
      Ok(handle) => {
        self.file_handles.insert(self.next_fh, handle);
        self.next_fh += 1;
        reply.opened(self.next_fh - 1, 0);
      }
    }
  }

  fn read(&mut self, _req: &fuse::Request, _ino: u64, fh: u64,
    offset: u64, size: u32, reply: fuse::ReplyData)
  {
    println!("read (ino {}, fh {}, offset {}, size {})", _ino, fh, offset, size);
    let res: Result<_, ext2::Error> = (|| {
      let handle = try!(self.file_handles.get_mut(&fh)
          .ok_or_else(|| ext2::Error::new(format!("Bad file handle"))));
      let mut buffer: Vec<u8> = iter::repeat(0).take(size as usize).collect();
      let length = try!(ext2::read_file(&mut self.fs, handle,
            offset, &mut buffer[..]));
      buffer.truncate(length as usize);
      Ok(buffer)
    })();

    match res {
      Err(_err) => reply.error(65),
      Ok(data) => reply.data(&data[..]),
    }
  }

  fn write(&mut self, _req: &fuse::Request, _ino: u64, fh: u64, offset: u64,
    data: &[u8], _flags: u32, reply: fuse::ReplyWrite)
  {
    println!("write (ino {}, fh {}, offset {}, size {})", _ino, fh, offset, data.len());
    let res: Result<_, ext2::Error> = (|| {
      let handle = try!(self.file_handles.get_mut(&fh)
          .ok_or_else(|| ext2::Error::new(format!("Bad file handle"))));
      let length = try!(ext2::write_file(&mut self.fs, handle, offset, data));
      Ok(length)
    })();

    match res {
      Err(_err) => { println!("{:?}", _err); reply.error(65) },
      Ok(length) => reply.written(length as u32),
    }
  }

  fn release(&mut self, _req: &fuse::Request, _ino: u64, fh: u64,
    _flags: u32, _lock_owner: u64, _flush: bool, reply: fuse::ReplyEmpty)
  {
    println!("release (ino {}, fh {})", _ino, fh);
    let res: Result<_, ext2::Error> = (|| {
      match self.file_handles.remove(&fh) {
        Some(handle) => ext2::close_file(&mut self.fs, handle),
        None => Ok(()),
      }
    })();

    match res {
      Err(_err) => reply.error(65),
      Ok(()) => reply.ok(),
    }
  }

  fn opendir(&mut self, _req: &fuse::Request, ino: u64,
    _flags: u32, reply: fuse::ReplyOpen)
  {
    println!("opendir (ino {})", ino);
    match ext2::open_dir(&mut self.fs, ext2_ino(ino)) {
      Err(_err) => reply.error(65),
      Ok(dir_handle) => {
        self.dir_handles.insert(self.next_fh, dir_handle);
        self.next_fh += 1;
        reply.opened(self.next_fh - 1, 0)
      },
    }
  }

  fn readdir(&mut self, _req: &fuse::Request, ino: u64, fh: u64,
    offset: u64, mut reply: fuse::ReplyDirectory)
  {
    println!("readdir (ino {}, fh {}, offset {})", ino, fh, offset);
    let res: Result<_, ext2::Error> = (|| {
      let handle = try!(self.dir_handles.get_mut(&fh)
          .ok_or_else(|| ext2::Error::new(format!("Bad dir handle"))));

      while let Some((next_handle, line)) =
        try!(ext2::read_dir(&mut self.fs, *handle)) 
      {
        let ino = fuse_ino(line.ino);
        let file_type = fuse_file_type(line.file_type);
        let name = <OsStr as OsStrExt>::from_bytes(&line.name[..]);
        if reply.add(ino, 0, file_type, name) {
          break
        } else {
          *handle = next_handle;
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
    println!("releasedir (ino {}, fh {})", _ino, fh);
    let res: Result<_, ext2::Error> = (|| {
      match self.dir_handles.remove(&fh) {
        Some(handle) => ext2::close_dir(&mut self.fs, handle),
        None => Ok(()),
      }
    })();

    match res {
      Err(_err) => reply.error(65),
      Ok(()) => reply.ok(),
    }
  }
}

fn mein() -> Result<(), ext2::Error> {
  let file = try!(fs::OpenOptions::new()
      .read(true).write(true).open("test.ext2"));
  let volume = ext2::FileVolume(file);
  let fs = try!(ext2::mount_fs(Box::new(volume)));
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

fn ext2_mode(mode: u16) -> Result<ext2::Mode, ext2::Error> {
  ext2::inode_mode_from_linux_mode(mode)
}

fn fuse_ino(ext2_ino: u64) -> u64 {
  if ext2_ino == ext2::Filesystem::ROOT_INO { 1 } else { ext2_ino }
}

fn inode_to_file_attr(inode: &ext2::Inode) -> fuse::FileAttr {
  fuse::FileAttr {
    ino: inode.ino,
    size: inode.size,
    blocks: inode.size_512 as u64,
    atime: fuse_timespec(inode.atime),
    ctime: fuse_timespec(inode.ctime),
    mtime: fuse_timespec(inode.mtime),
    crtime: fuse_timespec(0),
    kind: fuse_file_type(inode.mode.file_type),
    perm: inode.mode.access_rights,
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

