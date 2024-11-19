use crate::utils::Error;
use crate::elf::format::ElfFormat;
use crate::elf::constants::*;

#[derive(Debug, Clone)]
pub struct Symbol {
    pub st_name: u32,
    pub st_value: u32,
    pub st_size: u32,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
}

impl Symbol {
    pub fn new(fmt: &ElfFormat, data: &[u8]) -> Result<Self, Error> {
        let (st_name, st_value, st_size, st_info, st_other, st_shndx) = fmt.unpack_symbol(data)?;
        Ok(Self {
            st_name,
            st_value,
            st_size,
            st_info,
            st_other,
            st_shndx,
        })
    }

    pub fn bind(&self) -> u8 {
        self.st_info >> 4
    }

    pub fn set_bind(&mut self, bind: u8) {
        self.st_info = (bind << 4) | (self.st_info & 0xf);
    }

    pub fn type_(&self) -> u8 {
        self.st_info & 0xf
    }

    pub fn set_type(&mut self, type_: u8) {
        self.st_info = (self.st_info & 0xf0) | (type_ & 0xf);
    }

    pub fn pack(&self, fmt: &ElfFormat) -> Vec<u8> {
        fmt.pack_symbol(self.st_name, self.st_value, self.st_size, 
            ((self.st_info as u16) << 8) | (self.st_other as u16), 
            self.st_shndx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_bind_type() {
        let mut symbol = Symbol {
            st_name: 0,
            st_value: 0,
            st_size: 0,
            st_info: 0x23,  // bind = 2, type = 3
            st_other: 0,
            st_shndx: 0,
        };

        assert_eq!(symbol.bind(), 2);
        assert_eq!(symbol.type_(), 3);

        symbol.set_bind(4);
        symbol.set_type(5);
        assert_eq!(symbol.bind(), 4);
        assert_eq!(symbol.type_(), 5);
    }

    #[test]
    fn test_symbol_pack_unpack() {
        let fmt = ElfFormat::new(true);
        let original = Symbol {
            st_name: 1,
            st_value: 2,
            st_size: 3,
            st_info: 0x23,
            st_other: 0x45,
            st_shndx: 0x6789,
        };

        let packed = original.pack(&fmt);
        let unpacked = Symbol::new(&fmt, &packed).unwrap();

        assert_eq!(unpacked.st_name, original.st_name);
        assert_eq!(unpacked.st_value, original.st_value);
        assert_eq!(unpacked.st_size, original.st_size);
        assert_eq!(unpacked.st_info, original.st_info);
        assert_eq!(unpacked.st_other, original.st_other);
        assert_eq!(unpacked.st_shndx, original.st_shndx);
    }
}
