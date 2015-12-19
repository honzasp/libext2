extern crate ext2;
use std::{error, fs, iter};

fn mein() -> Result<(), ext2::Error> {
  let file = try!(fs::File::open("test.ext2"));
  let reader = ext2::FileReader(file);
  let context = try!(ext2::Context::new(Box::new(reader)));
  println!("{:?}", context.superblock());

  let inode = try!(context.read_inode(7792));
  println!("{:?}", inode);

  let mut read_data = ext2::ReadData::new(&context, &inode);
  let mut buffer: Vec<u8> = iter::repeat(0).take(inode.size as usize).collect();
  let data_length = try!(read_data.read(0, &mut buffer[..]));
  println!("{:?}", &buffer[0..data_length as usize]);

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
