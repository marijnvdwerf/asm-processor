use crate::utils::Error;
use crate::elf::Symbol;
use std::convert::TryInto;

#[derive(Debug, Clone, Copy)]
pub struct ElfFormat {
    pub is_big_endian: bool,
}

impl ElfFormat {
    pub fn new(is_big_endian: bool) -> Self {
        Self { is_big_endian }
    }

    pub fn pack_u16(&self, value: u16) -> Vec<u8> {
        if self.is_big_endian {
            value.to_be_bytes().to_vec()
        } else {
            value.to_le_bytes().to_vec()
        }
    }

    pub fn pack_u32(&self, value: u32) -> Vec<u8> {
        if self.is_big_endian {
            value.to_be_bytes().to_vec()
        } else {
            value.to_le_bytes().to_vec()
        }
    }

    pub fn unpack_u16(&self, data: &[u8]) -> Result<u16, Error> {
        if data.len() < 2 {
            return Err(Error::InvalidFormat("Data too short for u16".into()));
        }
        let bytes: [u8; 2] = data[..2].try_into().unwrap();
        Ok(if self.is_big_endian {
            u16::from_be_bytes(bytes)
        } else {
            u16::from_le_bytes(bytes)
        })
    }

    pub fn unpack_u32(&self, data: &[u8]) -> Result<u32, Error> {
        if data.len() < 4 {
            return Err(Error::InvalidFormat("Data too short for u32".into()));
        }
        let bytes: [u8; 4] = data[..4].try_into().unwrap();
        Ok(if self.is_big_endian {
            u32::from_be_bytes(bytes)
        } else {
            u32::from_le_bytes(bytes)
        })
    }

    pub fn pack_tuple_u32_25(&self, tuple: (u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32)) -> Vec<u8> {
        let mut result = Vec::with_capacity(25 * 4);
        let values = [
            tuple.0, tuple.1, tuple.2, tuple.3, tuple.4, tuple.5, tuple.6, tuple.7, tuple.8, tuple.9,
            tuple.10, tuple.11, tuple.12, tuple.13, tuple.14, tuple.15, tuple.16, tuple.17, tuple.18, tuple.19,
            tuple.20, tuple.21, tuple.22, tuple.23, tuple.24,
        ];
        for value in values.iter() {
            result.extend_from_slice(&self.pack_u32(*value));
        }
        result
    }

    pub fn unpack_tuple_u32_25(&self, data: &[u8]) -> Result<(u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32), Error> {
        if data.len() < 25 * 4 {
            return Err(Error::InvalidFormat("Data too short for unpacking 25 u32s".into()));
        }

        let mut values = Vec::with_capacity(25);
        for i in 0..25 {
            values.push(self.unpack_u32(&data[i * 4..(i + 1) * 4])?);
        }

        Ok((
            values[0], values[1], values[2], values[3], values[4], values[5], values[6], values[7], values[8], values[9],
            values[10], values[11], values[12], values[13], values[14], values[15], values[16], values[17], values[18], values[19],
            values[20], values[21], values[22], values[23], values[24],
        ))
    }

    pub fn pack_tuple_u32_10(&self, tuple: (u32, u32, u32, u32, u32, u32, u32, u32, u32, u32)) -> Vec<u8> {
        let mut result = Vec::with_capacity(40);
        result.extend_from_slice(&self.pack_u32(tuple.0));
        result.extend_from_slice(&self.pack_u32(tuple.1));
        result.extend_from_slice(&self.pack_u32(tuple.2));
        result.extend_from_slice(&self.pack_u32(tuple.3));
        result.extend_from_slice(&self.pack_u32(tuple.4));
        result.extend_from_slice(&self.pack_u32(tuple.5));
        result.extend_from_slice(&self.pack_u32(tuple.6));
        result.extend_from_slice(&self.pack_u32(tuple.7));
        result.extend_from_slice(&self.pack_u32(tuple.8));
        result.extend_from_slice(&self.pack_u32(tuple.9));
        result
    }

    pub fn unpack_tuple_u32(&self, data: &[u8]) -> Result<(u32, u32), Error> {
        if data.len() < 8 {
            return Err(Error::InvalidFormat("Data too short for tuple".into()));
        }
        Ok((
            self.unpack_u32(&data[0..4])?,
            self.unpack_u32(&data[4..8])?,
        ))
    }

    pub fn unpack_tuple_u32_3(&self, data: &[u8]) -> Result<(u32, u32, u32), Error> {
        if data.len() < 12 {
            return Err(Error::InvalidFormat("Data too short for tuple".into()));
        }
        Ok((
            self.unpack_u32(&data[0..4])?,
            self.unpack_u32(&data[4..8])?,
            self.unpack_u32(&data[8..12])?,
        ))
    }

    pub fn pack_symbol(&self, symbol: &Symbol) -> Vec<u8> {
        let mut result = Vec::with_capacity(16);
        result.extend_from_slice(&self.pack_u32(symbol.st_name));
        result.extend_from_slice(&self.pack_u32(symbol.st_value));
        result.extend_from_slice(&self.pack_u32(symbol.st_size));
        let info_other = ((symbol.st_info as u16) << 8) | (symbol.st_other as u16);
        result.extend_from_slice(&self.pack_u16(info_other));
        result.extend_from_slice(&self.pack_u16(symbol.st_shndx));
        result
    }

    pub fn unpack_symbol(&self, data: &[u8]) -> Result<(u32, u32, u32, u8, u8, u16), Error> {
        if data.len() < 16 {
            return Err(Error::InvalidFormat("Symbol data too short".into()));
        }

        let st_name = self.unpack_u32(&data[0..4])?;
        let st_value = self.unpack_u32(&data[4..8])?;
        let st_size = self.unpack_u32(&data[8..12])?;
        let info_other = self.unpack_u16(&data[12..14])?;
        let st_info = (info_other >> 8) as u8;
        let st_other = (info_other & 0xff) as u8;
        let st_shndx = self.unpack_u16(&data[14..16])?;

        Ok((st_name, st_value, st_size, st_info, st_other, st_shndx))
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
