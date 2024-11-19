use crate::utils::Error;
use crate::elf::format::ElfFormat;
use crate::elf::constants::SHT_RELA;

#[derive(Debug, Clone)]
pub struct Relocation {
    pub r_offset: u32,
    pub r_info: u32,
    pub r_addend: Option<u32>,
}

impl Relocation {
    pub fn new(fmt: &ElfFormat, data: &[u8], sh_type: u32) -> Result<Self, Error> {
        if data.len() < 8 {
            return Err(Error::InvalidFormat("Relocation data too short".into()));
        }

        let r_offset = fmt.unpack_u32(&data[0..4])?;
        let r_info = fmt.unpack_u32(&data[4..8])?;
        let r_addend = if sh_type == SHT_RELA {
            if data.len() < 12 {
                return Err(Error::InvalidFormat("RELA data too short".into()));
            }
            Some(fmt.unpack_u32(&data[8..12])?)
        } else {
            None
        };

        Ok(Self {
            r_offset,
            r_info,
            r_addend,
        })
    }

    pub fn sym(&self) -> u32 {
        self.r_info >> 8
    }

    pub fn type_(&self) -> u8 {
        (self.r_info & 0xff) as u8
    }

    pub fn offset(&self) -> u32 {
        self.r_offset
    }

    pub fn to_bytes(&self, fmt: &ElfFormat) -> Vec<u8> {
        let mut result = fmt.pack_tuple_u32_10((
            self.r_offset,
            self.r_info,
            self.r_addend.unwrap_or(0),
            0, 0, 0, 0, 0, 0, 0
        ));
        if self.r_addend.is_none() {
            result.truncate(8);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relocation_parse() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(0x12345678)); // r_offset
        data.extend_from_slice(&fmt.pack_u32(0x9ABCDEF0)); // r_info

        let reloc = Relocation::new(&fmt, &data, 2).unwrap();
        assert_eq!(reloc.r_offset, 0x12345678);
        assert_eq!(reloc.r_info, 0x9ABCDEF0);
        assert_eq!(reloc.r_addend, None);
        assert_eq!(reloc.sym(), 0x9ABCDE);
        assert_eq!(reloc.type_(), 0xF0);
    }

    #[test]
    fn test_relocation_with_addend() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(0x12345678)); // r_offset
        data.extend_from_slice(&fmt.pack_u32(0x9ABCDEF0)); // r_info
        data.extend_from_slice(&fmt.pack_u32(0x11223344)); // r_addend

        let reloc = Relocation::new(&fmt, &data, SHT_RELA).unwrap();
        assert_eq!(reloc.r_offset, 0x12345678);
        assert_eq!(reloc.r_info, 0x9ABCDEF0);
        assert_eq!(reloc.r_addend, Some(0x11223344));
    }

    #[test]
    fn test_relocation_pack() {
        let fmt = ElfFormat::new(true);
        let reloc = Relocation {
            r_offset: 0x12345678,
            r_info: 0x9ABCDEF0,
            r_addend: Some(0x11223344),
        };

        let packed = reloc.to_bytes(&fmt);
        let unpacked = Relocation::new(&fmt, &packed, SHT_RELA).unwrap();

        assert_eq!(unpacked.r_offset, reloc.r_offset);
        assert_eq!(unpacked.r_info, reloc.r_info);
        assert_eq!(unpacked.r_addend, reloc.r_addend);
    }
}
