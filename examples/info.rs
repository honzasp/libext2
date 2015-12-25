extern crate ext2;
use std::{error, fs, str};

fn mein() -> Result<(), ext2::Error> {
  let file = try!(fs::File::open("test.ext2"));
  let volume = ext2::FileVolume(file);
  let mut fs = try!(ext2::Filesystem::new(Box::new(volume)));

  let root_inode = try!(fs.read_inode(ext2::Filesystem::ROOT_INO));
  println!("{:?}", root_inode);

  let mut root_handle = try!(fs.dir_open(root_inode));
  while let Some(line) = try!(fs.dir_read(&mut root_handle)) {
    println!("  {:?}: {}", line, str::from_utf8(&line.name[..]).unwrap());
  }

  let dir_ino = try!(fs.dir_lookup(root_inode, b"totem_destroyer")).unwrap();
  let dir_inode = try!(fs.read_inode(dir_ino));
  println!("{:?}", dir_inode);

  let mut dir_handle = try!(fs.dir_open(dir_inode));
  while let Some(line) = try!(fs.dir_read(&mut dir_handle)) {
    println!("  {:?}: {}", line, str::from_utf8(&line.name[..]).unwrap());
  }

  let hello_ino = try!(fs.dir_lookup(root_inode, b"hello.txt")).unwrap();
  let hello_inode = try!(fs.read_inode(hello_ino));
  println!("{:?}", hello_inode);
  let mut hello_handle = try!(fs.file_open(hello_inode));
  let mut buffer: Vec<u8> = (0..hello_inode.size).map(|_| 0).collect();
  let length = try!(fs.file_read(&mut hello_handle, 0, &mut buffer[..]));
  buffer.truncate(length as usize);
  println!("{:?}", str::from_utf8(&buffer[..]).unwrap());

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
