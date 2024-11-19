use crate::utils::Error;
use crate::elf::format::ElfFormat;
use crate::elf::constants::*;
use crate::elf::section::Section;

#[derive(Debug, Clone, Default)]
pub struct Symbol {
    pub st_name: u32,
    pub st_value: u32,
    pub st_size: u32,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
    pub name: String,
    pub visibility: u8,
    fmt: ElfFormat,
}

impl Symbol {
    pub fn new<T: Section>(fmt: &ElfFormat, data: &[u8], strtab: &T) -> Result<Self, Error> {
        if data.len() < 16 {
            return Err(Error::InvalidFormat("Symbol data too short".into()));
        }

        let st_name = fmt.unpack_u32(&data[0..4])?;
        let st_value = fmt.unpack_u32(&data[4..8])?;
        let st_size = fmt.unpack_u32(&data[8..12])?;
        let st_info = data[12];
        let st_other = data[13];
        let st_shndx = fmt.unpack_u16(&data[14..16])?;
        let name = strtab.lookup_str(st_name.try_into().unwrap())?;
        let visibility = st_other & 0x3;

        Ok(Self {
            st_name,
            st_value,
            st_size,
            st_info,
            st_other,
            st_shndx,
            name,
            visibility,
            fmt: fmt.clone(),
        })
    }

    pub fn from_parts<T: Section>(
        fmt: ElfFormat,
        st_name: u32,
        st_value: u32,
        st_size: u32,
        st_info: u8,
        st_other: u8,
        st_shndx: u16,
        strtab: &T,
        name: String,
    ) -> Result<Self, Error> {
        Ok(Self {
            st_name,
            st_value,
            st_size,
            st_info,
            st_other,
            st_shndx,
            name,
            visibility: st_other & 3,
            fmt,
        })
    }

    pub fn to_bin(&self) -> Result<Vec<u8>, Error> {
        self.to_bytes(&self.fmt)
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

    pub fn to_bytes(&self, fmt: &ElfFormat) -> Result<Vec<u8>, Error> {
        let mut result = vec![0; 16];
        fmt.pack_u32(&mut result[0..4], self.st_name)?;
        fmt.pack_u32(&mut result[4..8], self.st_value)?;
        fmt.pack_u32(&mut result[8..12], self.st_size)?;
        result[12] = self.st_info;
        result[13] = self.st_other;
        fmt.pack_u16(&mut result[14..16], self.st_shndx)?;
        Ok(result)
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
            name: String::new(),
            visibility: 0,
            fmt: ElfFormat::new(true),
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
            name: String::new(),
            visibility: 0,
            fmt: fmt.clone(),
        };

        let bytes = original.to_bytes(&fmt).unwrap();
        let symbol = Symbol::new(&fmt, &bytes, &Section::new(&fmt, &[])).unwrap();
        assert_eq!(symbol.st_name, original.st_name);
        assert_eq!(symbol.st_value, original.st_value);
        assert_eq!(symbol.st_size, original.st_size);
        assert_eq!(symbol.st_info, original.st_info);
        assert_eq!(symbol.st_other, original.st_other);
        assert_eq!(symbol.st_shndx, original.st_shndx);
    }
}
