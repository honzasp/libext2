pub struct FuseFS;

impl fuse::Filesystem for FuseFS {
  fn lookup(&mut self, _req: &fuse::Request,
    parent_ino: u64, name: &Path, reply: fuse::ReplyEntry)
  {
    // TODO: lookup file attributes in a directory
  }

  fn getattr(&mut self, _req: &fuse::Request, ino: u64, reply: fuse::ReplyAttr) {
    // TODO: lookup attributes of an inode
  }

  fn readlink(&mut self, _req: &fuse::Request, ino: u64, reply: fuse::ReplyData) {
    // TODO: read inode as a symbolic link
  }

  fn open(&mut self, _req: &fuse::Request, ino: u64, flags: u32, reply: fuse::ReplyOpen) {
    // TODO: "open" a file. the reply can specify a 64-bit file handle (fh) that
    // will be given back to us in read()/write(). this can be used as an index
    // or something.
  }

  fn read(&mut self, _req: &fuse::Request, ino: u64, fh: u64,
    offset: u64, size: u32, reply: fuse::ReplyData)
  {
    // should read exactly the number of bytes requested and send it to reply
  }

  fn release(&mut self, _req: &fuse::Request, ino: u64, fh: u64,
    flags: u32, lock_owner: u64, flush: bool, reply: fuse::ReplyEmpty)
  {
    // should drop the context initialized in open()
  }

  fn opendir(&mut self, _req: &fuse::Request, ino: u64,
    flags: u32, reply: fuse::ReplyOpen)
  {
    // open a directory, again a file handle
  }

  fn readdir(&mut self, _req: &fuse::Request, ino: u64, fh: u64,
    offset: u64, reply: fuse::ReplyDirectory)
  {
    // the offset can be some opaque value that has been passed to the prevous
    // reply?
  }

  fn releasedir(&mut self, _req: &fuse::Request, ino: u64, fh: u64,
    flags: u32, reply: fuse::ReplyEmpty)
  {
    // close an opened directory
  }
}
