extern crate ext2;
use std::{error, fs};

fn mein() -> Result<(), ext2::Error> {
  let file = try!(fs::File::open("test.ext2"));
  let reader = ext2::block_read::FileRead::new(file);
  let context = try!(ext2::ino::Context::new(Box::new(reader)));
  println!("{:?}", context.superblock);
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
