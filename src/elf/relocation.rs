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

    pub fn sym_index(&self) -> u32 {
        self.r_info >> 8
    }

    pub fn rel_type(&self) -> u8 {
        (self.r_info & 0xff) as u8
    }

    pub fn set_sym_index(&mut self, index: u32) {
        self.r_info = (index << 8) | (self.r_info & 0xff);
    }

    pub fn set_rel_type(&mut self, type_: u8) {
        self.r_info = (self.r_info & !0xff) | (type_ as u32);
    }

    pub fn to_bytes(&self, fmt: &ElfFormat) -> Vec<u8> {
        let mut result = vec![0; if self.r_addend.is_some() { 12 } else { 8 }];
        fmt.pack_u32(&mut result[0..4], self.r_offset).unwrap();
        fmt.pack_u32(&mut result[4..8], self.r_info).unwrap();
        if let Some(addend) = self.r_addend {
            fmt.pack_u32(&mut result[8..12], addend).unwrap();
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
        let mut data = vec![0; 8];
        fmt.pack_u32(&mut data[0..4], 0x12345678).unwrap(); // r_offset
        fmt.pack_u32(&mut data[4..8], 0x9ABCDEF0).unwrap(); // r_info

        let reloc = Relocation::new(&fmt, &data, 2).unwrap();
        assert_eq!(reloc.r_offset, 0x12345678);
        assert_eq!(reloc.r_info, 0x9ABCDEF0);
        assert_eq!(reloc.r_addend, None);
        assert_eq!(reloc.sym_index(), 0x9ABCDE);
        assert_eq!(reloc.rel_type(), 0xF0);
    }

    #[test]
    fn test_relocation_with_addend() {
        let fmt = ElfFormat::new(true);
        let mut data = vec![0; 12];
        fmt.pack_u32(&mut data[0..4], 0x12345678).unwrap(); // r_offset
        fmt.pack_u32(&mut data[4..8], 0x9ABCDEF0).unwrap(); // r_info
        fmt.pack_u32(&mut data[8..12], 0x11223344).unwrap(); // r_addend

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

        let bytes = reloc.to_bytes(&fmt);
        let reloc2 = Relocation::new(&fmt, &bytes, SHT_RELA).unwrap();
        assert_eq!(reloc.r_offset, reloc2.r_offset);
        assert_eq!(reloc.r_info, reloc2.r_info);
        assert_eq!(reloc.r_addend, reloc2.r_addend);
    }
}
