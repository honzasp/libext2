use std::{convert, error, fmt, io, result};

#[derive(Debug)]
pub struct Error {
  message: String,
  cause: Option<Box<error::Error>>,
}

pub type Result<T> = result::Result<T, Error>;

impl Error {
  pub fn new(message: String) -> Error {
    Error { message: message, cause: None }
  }
}

impl error::Error for Error {
  fn description(&self) -> &str {
    &self.message[..]
  }

  fn cause(&self) -> Option<&error::Error> {
    self.cause.as_ref().map(|e| &**e)
  }
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
    f.write_str(&self.message[..])
  }
}

impl convert::From<io::Error> for Error {
  fn from(err: io::Error) -> Error {
    Error { message: format!("IO error"), cause: Some(Box::new(err)) }
  }
}
