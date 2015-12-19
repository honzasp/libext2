pub struct DirReader<'i> {
  data_reader: DataReader<'i>
}

impl<'i> DirReader<'i> {
  pub fn seek_entry(&mut self, name: &[u8]) -> io::Result<DirEntry>;
  pub fn next_entry(&mut self) -> io::Result<Option<DirEntry>>;
}

pub struct DirEntry {
  pub inode: u64,
  pub name: Vec<u8>,
  pub file_type: FileType,
}

