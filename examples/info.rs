extern crate ext2;
use std::{error, fs, str};

fn mein() -> Result<(), ext2::Error> {
  let file = try!(fs::File::open("test.ext2"));
  let reader = ext2::FileReader(file);
  let context = try!(ext2::Context::new(Box::new(reader)));
  println!("{:?}", context.superblock());

  let inode = try!(context.read_inode(7329));
  println!("{:?}", inode);

  let mut cursor = try!(ext2::read_dir::init_cursor(&context, &inode));
  while let Some(entry) =
    try!(ext2::read_dir::advance_cursor(&context, &mut cursor)) 
  {
    let name = str::from_utf8(&entry.name[..]).unwrap();
    println!("  entry {:?} {:?}", name, entry);
  }

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
