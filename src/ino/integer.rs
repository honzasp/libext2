pub fn read_u16(bytes: &[u8]) -> u16 {
  (bytes[0] as u16) +
  ((bytes[1] as u16) << 8)
}

pub fn read_u32(bytes: &[u8]) -> u32 {
  (bytes[0] as u32) +
  ((bytes[1] as u32) << 8) +
  ((bytes[2] as u32) << 16) +
  ((bytes[3] as u32) << 24)
}
