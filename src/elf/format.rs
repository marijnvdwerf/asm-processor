use byteorder::{BigEndian, ByteOrder, LittleEndian};

#[derive(Debug, Clone, Copy)]
pub struct ElfFormat {
    pub is_big_endian: bool,
}

impl ElfFormat {
    pub fn new(is_big_endian: bool) -> Self {
        Self { is_big_endian }
    }

    pub fn pack<T: ByteOrder>(&self, data: &[u8]) -> Vec<u8> {
        data.to_vec()
    }

    pub fn pack_u16(&self, val: u16) -> [u8; 2] {
        let mut bytes = [0; 2];
        if self.is_big_endian {
            BigEndian::write_u16(&mut bytes, val);
        } else {
            LittleEndian::write_u16(&mut bytes, val);
        }
        bytes
    }

    pub fn pack_u32(&self, val: u32) -> [u8; 4] {
        let mut bytes = [0; 4];
        if self.is_big_endian {
            BigEndian::write_u32(&mut bytes, val);
        } else {
            LittleEndian::write_u32(&mut bytes, val);
        }
        bytes
    }

    pub fn unpack_u16(&self, data: &[u8]) -> u16 {
        if self.is_big_endian {
            BigEndian::read_u16(data)
        } else {
            LittleEndian::read_u16(data)
        }
    }

    pub fn unpack_u32(&self, data: &[u8]) -> u32 {
        if self.is_big_endian {
            BigEndian::read_u32(data)
        } else {
            LittleEndian::read_u32(data)
        }
    }

    pub fn unpack_multiple_u32(&self, data: &[u8]) -> Vec<u32> {
        let mut result = Vec::with_capacity(data.len() / 4);
        for chunk in data.chunks(4) {
            result.push(self.unpack_u32(chunk));
        }
        result
    }

    pub fn unpack_u8(&self, data: &[u8]) -> u8 {
        data[0]
    }

    pub fn pack_multiple(&self, values: &[u32]) -> Vec<u8> {
        let mut result = Vec::with_capacity(values.len() * 4);
        for &val in values {
            result.extend_from_slice(&self.pack_u32(val));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack_u32_big_endian() {
        let fmt = ElfFormat::new(true);
        let val = 0x12345678;
        let packed = fmt.pack_u32(val);
        assert_eq!(packed, [0x12, 0x34, 0x56, 0x78]);
        let unpacked = fmt.unpack_u32(&packed);
        assert_eq!(unpacked, val);
    }

    #[test]
    fn test_pack_unpack_u32_little_endian() {
        let fmt = ElfFormat::new(false);
        let val = 0x12345678;
        let packed = fmt.pack_u32(val);
        assert_eq!(packed, [0x78, 0x56, 0x34, 0x12]);
        let unpacked = fmt.unpack_u32(&packed);
        assert_eq!(unpacked, val);
    }

    #[test]
    fn test_pack_unpack_u16() {
        let fmt = ElfFormat::new(true);
        let val = 0x1234;
        let packed = fmt.pack_u16(val);
        assert_eq!(packed, [0x12, 0x34]);
        let unpacked = fmt.unpack_u16(&packed);
        assert_eq!(unpacked, val);
    }

    #[test]
    fn test_pack_multiple() {
        let fmt = ElfFormat::new(true);
        let values = vec![0x12345678, 0x9ABCDEF0];
        let packed = fmt.pack_multiple(&values);
        assert_eq!(packed, vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0]);
        let unpacked = fmt.unpack_multiple_u32(&packed);
        assert_eq!(unpacked, values);
    }
}
