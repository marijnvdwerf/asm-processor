use crate::elf::format::ElfFormat;
use crate::elf::constants::SHT_REL;

#[derive(Debug, Clone)]
pub struct Relocation {
    pub r_offset: u32,
    pub r_info: u32,
    pub r_addend: Option<u32>,
    pub sym_index: u32,
    pub rel_type: u8,
    fmt: ElfFormat,
    sh_type: u32,
}

impl Relocation {
    pub fn new(fmt: ElfFormat, data: &[u8], sh_type: u32) -> Self {
        let (r_offset, r_info, r_addend) = if sh_type == SHT_REL {
            let (offset, info) = fmt.unpack_tuple_u32(data);
            (offset, info, None)
        } else {
            let (offset, info, addend) = fmt.unpack_tuple_u32_3(data);
            (offset, info, Some(addend))
        };

        let sym_index = r_info >> 8;
        let rel_type = (r_info & 0xff) as u8;

        Self {
            r_offset,
            r_info,
            r_addend,
            sym_index,
            rel_type,
            fmt,
            sh_type,
        }
    }

    pub fn to_bytes(&mut self) -> Vec<u8> {
        self.r_info = (self.sym_index << 8) | (self.rel_type as u32);
        
        if self.sh_type == SHT_REL {
            let mut result = Vec::with_capacity(8);
            result.extend_from_slice(&self.fmt.pack_u32(self.r_offset));
            result.extend_from_slice(&self.fmt.pack_u32(self.r_info));
            result
        } else {
            let mut result = Vec::with_capacity(12);
            result.extend_from_slice(&self.fmt.pack_u32(self.r_offset));
            result.extend_from_slice(&self.fmt.pack_u32(self.r_info));
            result.extend_from_slice(&self.fmt.pack_u32(self.r_addend.unwrap_or(0)));
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rel_parse() {
        let fmt = ElfFormat::new(true); // big-endian
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(0x1000)); // r_offset
        data.extend_from_slice(&fmt.pack_u32(0x0205)); // r_info (sym_index = 2, type = 5)

        let rel = Relocation::new(fmt, &data, SHT_REL);
        assert_eq!(rel.r_offset, 0x1000);
        assert_eq!(rel.sym_index, 2);
        assert_eq!(rel.rel_type, 5);
        assert!(rel.r_addend.is_none());
    }

    #[test]
    fn test_rela_parse() {
        let fmt = ElfFormat::new(true); // big-endian
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(0x2000)); // r_offset
        data.extend_from_slice(&fmt.pack_u32(0x0408)); // r_info (sym_index = 4, type = 8)
        data.extend_from_slice(&fmt.pack_u32(0x42)); // r_addend

        let rel = Relocation::new(fmt, &data, SHT_REL + 1); // SHT_RELA
        assert_eq!(rel.r_offset, 0x2000);
        assert_eq!(rel.sym_index, 4);
        assert_eq!(rel.rel_type, 8);
        assert_eq!(rel.r_addend, Some(0x42));
    }

    #[test]
    fn test_rel_roundtrip() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(0x1000));
        data.extend_from_slice(&fmt.pack_u32(0x0205));

        let mut rel = Relocation::new(fmt, &data, SHT_REL);
        let bytes = rel.to_bytes();
        assert_eq!(data, bytes);
    }

    #[test]
    fn test_rela_roundtrip() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(0x2000));
        data.extend_from_slice(&fmt.pack_u32(0x0408));
        data.extend_from_slice(&fmt.pack_u32(0x42));

        let mut rel = Relocation::new(fmt, &data, SHT_REL + 1);
        let bytes = rel.to_bytes();
        assert_eq!(data, bytes);
    }
}
