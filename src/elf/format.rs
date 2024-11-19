use crate::utils::Error;
use crate::elf::Symbol;
use std::convert::TryInto;

#[derive(Debug, Clone, Copy)]
pub struct ElfFormat {
    pub is_big_endian: bool,
}

impl Default for ElfFormat {
    fn default() -> Self {
        Self {
            is_big_endian: false
        }
    }
}

impl ElfFormat {
    pub fn new(is_big_endian: bool) -> Self {
        Self { is_big_endian }
    }

    pub fn pack_u16(&self, data: &mut [u8], value: u16) -> Result<(), Error> {
        if data.len() < 2 {
            return Err(Error::InvalidFormat("Data too short for packing u16".to_string()));
        }
        let bytes = if self.is_big_endian {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        };
        data[0..2].copy_from_slice(&bytes);
        Ok(())
    }

    pub fn unpack_u16(&self, data: &[u8]) -> Result<u16, Error> {
        if data.len() < 2 {
            return Err(Error::InvalidFormat("Data too short for unpacking u16".to_string()));
        }
        Ok(if self.is_big_endian {
            u16::from_be_bytes([data[0], data[1]])
        } else {
            u16::from_le_bytes([data[0], data[1]])
        })
    }

    pub fn pack_u32(&self, data: &mut [u8], value: u32) -> Result<(), Error> {
        if data.len() < 4 {
            return Err(Error::InvalidFormat("Data too short for packing u32".to_string()));
        }
        let bytes = if self.is_big_endian {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        };
        data[0..4].copy_from_slice(&bytes);
        Ok(())
    }

    pub fn unpack_u32(&self, data: &[u8]) -> Result<u32, Error> {
        if data.len() < 4 {
            return Err(Error::InvalidFormat("Data too short for unpacking u32".to_string()));
        }
        let bytes: [u8; 4] = data[..4].try_into().unwrap();
        Ok(if self.is_big_endian {
            u32::from_be_bytes(bytes)
        } else {
            u32::from_le_bytes(bytes)
        })
    }

    pub fn pack_tuple_u32(&self, data: &mut [u8], values: &[u32]) -> Result<(), Error> {
        if data.len() < values.len() * 4 {
            return Err(Error::InvalidFormat(format!("Data too short for packing {} u32s", values.len())));
        }
        for (i, &value) in values.iter().enumerate() {
            self.pack_u32(&mut data[i*4..(i+1)*4], value)?;
        }
        Ok(())
    }

    pub fn unpack_tuple_u32(&self, data: &[u8], count: usize) -> Result<Vec<u32>, Error> {
        if data.len() < count * 4 {
            return Err(Error::InvalidFormat(format!("Data too short for unpacking {} u32s", count)));
        }
        let mut result = Vec::with_capacity(count);
        for i in 0..count {
            result.push(self.unpack_u32(&data[i*4..(i+1)*4])?);
        }
        Ok(result)
    }

    pub fn pack_symbol(&self, symbol: &Symbol) -> Result<Vec<u8>, Error> {
        let mut result = vec![0; 16];
        self.pack_u32(&mut result[0..4], symbol.st_name)?;
        self.pack_u32(&mut result[4..8], symbol.st_value)?;
        self.pack_u32(&mut result[8..12], symbol.st_size)?;
        result[12] = symbol.st_info;
        result[13] = symbol.st_other;
        self.pack_u16(&mut result[14..16], symbol.st_shndx)?;
        Ok(result)
    }

    pub fn unpack_symbol(&self, data: &[u8]) -> Result<(u32, u32, u32, u8, u8, u16), Error> {
        if data.len() < 16 {
            return Err(Error::InvalidFormat("Symbol data too short".to_string()));
        }

        let st_name = self.unpack_u32(&data[0..4])?;
        let st_value = self.unpack_u32(&data[4..8])?;
        let st_size = self.unpack_u32(&data[8..12])?;
        let st_info = data[12];
        let st_other = data[13];
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
        let mut data = [0; 2];
        fmt.pack_u16(&mut data, value).unwrap();
        let unpacked = fmt.unpack_u16(&data).unwrap();
        assert_eq!(value, unpacked);
    }

    #[test]
    fn test_pack_unpack_u32_big_endian() {
        let fmt = ElfFormat::new(true);
        let value = 0x12345678;
        let mut data = [0; 4];
        fmt.pack_u32(&mut data, value).unwrap();
        let unpacked = fmt.unpack_u32(&data).unwrap();
        assert_eq!(value, unpacked);
    }

    #[test]
    fn test_pack_unpack_u32_little_endian() {
        let fmt = ElfFormat::new(false);
        let value = 0x12345678;
        let mut data = [0; 4];
        fmt.pack_u32(&mut data, value).unwrap();
        let unpacked = fmt.unpack_u32(&data).unwrap();
        assert_eq!(value, unpacked);
    }

    #[test]
    fn test_pack_multiple() {
        let fmt = ElfFormat::new(true);
        let mut result = Vec::new();
        let mut data1 = [0; 4];
        let mut data2 = [0; 4];
        let mut data3 = [0; 4];
        fmt.pack_u32(&mut data1, 1).unwrap();
        fmt.pack_u32(&mut data2, 2).unwrap();
        fmt.pack_u32(&mut data3, 3).unwrap();
        result.extend_from_slice(&data1);
        result.extend_from_slice(&data2);
        result.extend_from_slice(&data3);
        assert_eq!(result.len(), 12);
    }

    #[test]
    fn test_unpack_tuple_u32() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        let mut data1 = [0; 4];
        let mut data2 = [0; 4];
        fmt.pack_u32(&mut data1, 1).unwrap();
        fmt.pack_u32(&mut data2, 2).unwrap();
        data.extend_from_slice(&data1);
        data.extend_from_slice(&data2);
        let unpacked = fmt.unpack_tuple_u32(&data, 2).unwrap();
        assert_eq!(unpacked.len(), 2);
        assert_eq!(unpacked[0], 1);
        assert_eq!(unpacked[1], 2);
    }

    #[test]
    fn test_unpack_symbol() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        let mut data1 = [0; 4];
        let mut data2 = [0; 4];
        let mut data3 = [0; 4];
        let mut data4 = [0; 2];
        fmt.pack_u32(&mut data1, 1).unwrap();
        fmt.pack_u32(&mut data2, 2).unwrap();
        fmt.pack_u32(&mut data3, 3).unwrap();
        fmt.pack_u16(&mut data4, 0x5678).unwrap();
        data.extend_from_slice(&data1);
        data.extend_from_slice(&data2);
        data.extend_from_slice(&data3);
        data.push(0x12);
        data.push(0x34);
        data.extend_from_slice(&data4);
        let (st_name, st_value, st_size, st_info, st_other, st_shndx) = fmt.unpack_symbol(&data).unwrap();
        assert_eq!(st_name, 1);
        assert_eq!(st_value, 2);
        assert_eq!(st_size, 3);
        assert_eq!(st_info, 0x12);
        assert_eq!(st_other, 0x34);
        assert_eq!(st_shndx, 0x5678);
    }
}
