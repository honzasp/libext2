pub fn encode_u32(bytes: &mut [u8], value: u32) {
  bytes[0] = (value & 0xff) as u8;
  bytes[1] = ((value >> 8) & 0xff) as u8;
  bytes[2] = ((value >> 16) & 0xff) as u8;
  bytes[3] = ((value >> 24) & 0xff) as u8;
}
