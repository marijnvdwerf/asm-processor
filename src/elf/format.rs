use crate::utils::Error;
use byteorder::{BigEndian, ByteOrder, LittleEndian};

#[derive(Debug, Clone, Copy)]
pub struct ElfFormat {
    big_endian: bool,
}

impl ElfFormat {
    pub fn new(big_endian: bool) -> Self {
        Self { big_endian }
    }

    pub fn default() -> Self {
        Self { big_endian: true }
    }

    pub fn pack_u16(&self, value: u16) -> [u8; 2] {
        let mut buf = [0; 2];
        if self.big_endian {
            BigEndian::write_u16(&mut buf, value);
        } else {
            LittleEndian::write_u16(&mut buf, value);
        }
        buf
    }

    pub fn pack_u32(&self, value: u32) -> [u8; 4] {
        let mut buf = [0; 4];
        if self.big_endian {
            BigEndian::write_u32(&mut buf, value);
        } else {
            LittleEndian::write_u32(&mut buf, value);
        }
        buf
    }

    pub fn unpack_u16(&self, data: &[u8]) -> Result<u16, Error> {
        if data.len() < 2 {
            return Err(Error::InvalidFormat("Data too short for u16".into()));
        }
        Ok(if self.big_endian {
            BigEndian::read_u16(data)
        } else {
            LittleEndian::read_u16(data)
        })
    }

    pub fn unpack_u32(&self, data: &[u8]) -> Result<u32, Error> {
        if data.len() < 4 {
            return Err(Error::InvalidFormat("Data too short for u32".into()));
        }
        Ok(if self.big_endian {
            BigEndian::read_u32(data)
        } else {
            LittleEndian::read_u32(data)
        })
    }

    pub fn unpack_tuple_u32(&self, data: &[u8]) -> Result<(u32, u32), Error> {
        if data.len() < 8 {
            return Err(Error::InvalidFormat("Data too short for u32 tuple".into()));
        }
        Ok((
            self.unpack_u32(&data[0..4])?,
            self.unpack_u32(&data[4..8])?,
        ))
    }

    pub fn unpack_tuple_u32_3(&self, data: &[u8]) -> Result<(u32, u32, u32), Error> {
        if data.len() < 12 {
            return Err(Error::InvalidFormat("Data too short for u32 tuple".into()));
        }
        Ok((
            self.unpack_u32(&data[0..4])?,
            self.unpack_u32(&data[4..8])?,
            self.unpack_u32(&data[8..12])?,
        ))
    }

    pub fn unpack_symbol(&self, data: &[u8]) -> Result<(u32, u32, u32, u8, u8, u16), Error> {
        if data.len() < 16 {
            return Err(Error::InvalidFormat("Data too short for symbol".into()));
        }
        Ok((
            self.unpack_u32(&data[0..4])?,   // st_name
            self.unpack_u32(&data[4..8])?,   // st_value
            self.unpack_u32(&data[8..12])?,  // st_size
            data[12],                       // st_info
            data[13],                       // st_other
            self.unpack_u16(&data[14..16])?, // st_shndx
        ))
    }

    pub fn unpack_tuple_u32_10(&self, data: &[u8]) -> Result<(u32, u32, u32, u32, u32, u32, u32, u32, u32, u32), Error> {
        if data.len() < 40 {
            return Err(Error::InvalidFormat("Data too short for u32 tuple".into()));
        }
        Ok((
            self.unpack_u32(&data[0..4])?,
            self.unpack_u32(&data[4..8])?,
            self.unpack_u32(&data[8..12])?,
            self.unpack_u32(&data[12..16])?,
            self.unpack_u32(&data[16..20])?,
            self.unpack_u32(&data[20..24])?,
            self.unpack_u32(&data[24..28])?,
            self.unpack_u32(&data[28..32])?,
            self.unpack_u32(&data[32..36])?,
            self.unpack_u32(&data[36..40])?,
        ))
    }

    pub fn pack_tuple_u32_10(
        &self,
        v1: u32,
        v2: u32,
        v3: u32,
        v4: u32,
        v5: u32,
        v6: u32,
        v7: u32,
        v8: u32,
        v9: u32,
        v10: u32,
    ) -> Vec<u8> {
        let mut result = Vec::with_capacity(40);
        result.extend_from_slice(&self.pack_u32(v1));
        result.extend_from_slice(&self.pack_u32(v2));
        result.extend_from_slice(&self.pack_u32(v3));
        result.extend_from_slice(&self.pack_u32(v4));
        result.extend_from_slice(&self.pack_u32(v5));
        result.extend_from_slice(&self.pack_u32(v6));
        result.extend_from_slice(&self.pack_u32(v7));
        result.extend_from_slice(&self.pack_u32(v8));
        result.extend_from_slice(&self.pack_u32(v9));
        result.extend_from_slice(&self.pack_u32(v10));
        result
    }

    pub fn unpack_tuple_u32_25(&self, data: &[u8]) -> Result<(u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32), Error> {
        if data.len() < 100 {
            return Err(Error::InvalidFormat("Data too short for u32 tuple".into()));
        }
        Ok((
            self.unpack_u32(&data[0..4])?,
            self.unpack_u32(&data[4..8])?,
            self.unpack_u32(&data[8..12])?,
            self.unpack_u32(&data[12..16])?,
            self.unpack_u32(&data[16..20])?,
            self.unpack_u32(&data[20..24])?,
            self.unpack_u32(&data[24..28])?,
            self.unpack_u32(&data[28..32])?,
            self.unpack_u32(&data[32..36])?,
            self.unpack_u32(&data[36..40])?,
            self.unpack_u32(&data[40..44])?,
            self.unpack_u32(&data[44..48])?,
            self.unpack_u32(&data[48..52])?,
            self.unpack_u32(&data[52..56])?,
            self.unpack_u32(&data[56..60])?,
            self.unpack_u32(&data[60..64])?,
            self.unpack_u32(&data[64..68])?,
            self.unpack_u32(&data[68..72])?,
            self.unpack_u32(&data[72..76])?,
            self.unpack_u32(&data[76..80])?,
            self.unpack_u32(&data[80..84])?,
            self.unpack_u32(&data[84..88])?,
            self.unpack_u32(&data[88..92])?,
            self.unpack_u32(&data[92..96])?,
            self.unpack_u32(&data[96..100])?,
        ))
    }

    pub fn pack_symbol(&self, st_name: u32, st_value: u32, st_size: u32, info_other: u16, st_shndx: u16) -> Vec<u8> {
        let mut result = Vec::with_capacity(16);
        result.extend_from_slice(&self.pack_u32(st_name));
        result.extend_from_slice(&self.pack_u32(st_value));
        result.extend_from_slice(&self.pack_u32(st_size));
        result.extend_from_slice(&self.pack_u16(info_other));
        result.extend_from_slice(&self.pack_u16(st_shndx));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack_u16() {
        let fmt = ElfFormat::new(true);
        let value = 0x1234;
        let packed = fmt.pack_u16(value);
        let unpacked = fmt.unpack_u16(&packed).unwrap();
        assert_eq!(value, unpacked);
    }

    #[test]
    fn test_pack_unpack_u32_big_endian() {
        let fmt = ElfFormat::new(true);
        let value = 0x12345678;
        let packed = fmt.pack_u32(value);
        let unpacked = fmt.unpack_u32(&packed).unwrap();
        assert_eq!(value, unpacked);
    }

    #[test]
    fn test_pack_unpack_u32_little_endian() {
        let fmt = ElfFormat::new(false);
        let value = 0x12345678;
        let packed = fmt.pack_u32(value);
        let unpacked = fmt.unpack_u32(&packed).unwrap();
        assert_eq!(value, unpacked);
    }

    #[test]
    fn test_pack_multiple() {
        let fmt = ElfFormat::new(true);
        let mut result = Vec::new();
        result.extend_from_slice(&fmt.pack_u32(1));
        result.extend_from_slice(&fmt.pack_u32(2));
        result.extend_from_slice(&fmt.pack_u32(3));
        assert_eq!(result.len(), 12);
    }

    #[test]
    fn test_unpack_tuple_u32() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(1));
        data.extend_from_slice(&fmt.pack_u32(2));
        let (v1, v2) = fmt.unpack_tuple_u32(&data).unwrap();
        assert_eq!(v1, 1);
        assert_eq!(v2, 2);
    }

    #[test]
    fn test_unpack_tuple_u32_3() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(1));
        data.extend_from_slice(&fmt.pack_u32(2));
        data.extend_from_slice(&fmt.pack_u32(3));
        let (v1, v2, v3) = fmt.unpack_tuple_u32_3(&data).unwrap();
        assert_eq!(v1, 1);
        assert_eq!(v2, 2);
        assert_eq!(v3, 3);
    }

    #[test]
    fn test_unpack_symbol() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(1)); // st_name
        data.extend_from_slice(&fmt.pack_u32(2)); // st_value
        data.extend_from_slice(&fmt.pack_u32(3)); // st_size
        data.extend_from_slice(&[0x12]); // st_info
        data.extend_from_slice(&[0x34]); // st_other
        data.extend_from_slice(&fmt.pack_u16(0x5678)); // st_shndx
        let (st_name, st_value, st_size, st_info, st_other, st_shndx) = fmt.unpack_symbol(&data).unwrap();
        assert_eq!(st_name, 1);
        assert_eq!(st_value, 2);
        assert_eq!(st_size, 3);
        assert_eq!(st_info, 0x12);
        assert_eq!(st_other, 0x34);
        assert_eq!(st_shndx, 0x5678);
    }
}
