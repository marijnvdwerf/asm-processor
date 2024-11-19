use crate::elf::format::ElfFormat;
use crate::elf::constants::SHN_XINDEX;
use crate::elf::section::Section;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SymbolError {
    #[error("SHN_XINDEX not supported (too many sections)")]
    XindexNotSupported,
}

/// Represents an ELF symbol table entry
/// 
/// ```c
/// typedef struct {
///     Elf32_Word      st_name;
///     Elf32_Addr      st_value;
///     Elf32_Word      st_size;
///     unsigned char   st_info;
///     unsigned char   st_other;
///     Elf32_Half      st_shndx;
/// } Elf32_Sym;
/// ```
#[derive(Debug, Clone)]
pub struct Symbol {
    pub st_name: u32,
    pub st_value: u32,
    pub st_size: u32,
    pub st_other: u8,
    pub st_shndx: u16,
    pub bind: u8,
    pub type_: u8,
    pub visibility: u8,
    pub name: String,
    fmt: ElfFormat,
}

impl Symbol {
    pub fn new<S: Section>(fmt: ElfFormat, data: &[u8], strtab: &S, name: Option<String>) -> Result<Self, SymbolError> {
        let (st_name, st_value, st_size, st_info, st_other, st_shndx) = fmt.unpack_symbol(data);

        if st_shndx == SHN_XINDEX {
            return Err(SymbolError::XindexNotSupported);
        }

        let bind = st_info >> 4;
        let type_ = st_info & 15;
        let visibility = st_other & 3;

        Ok(Self {
            st_name,
            st_value,
            st_size,
            st_other,
            st_shndx,
            bind,
            type_,
            visibility,
            name: name.unwrap_or_else(|| strtab.lookup_str(st_name)),
            fmt,
        })
    }

    pub fn from_parts<S: Section>(
        fmt: ElfFormat,
        st_name: u32,
        st_value: u32,
        st_size: u32,
        st_info: u8,
        st_other: u8,
        st_shndx: u16,
        strtab: &S,
        name: String,
    ) -> Result<Self, SymbolError> {
        if st_shndx == SHN_XINDEX {
            return Err(SymbolError::XindexNotSupported);
        }

        let bind = st_info >> 4;
        let type_ = st_info & 15;
        let visibility = st_other & 3;

        Ok(Self {
            st_name,
            st_value,
            st_size,
            st_other,
            st_shndx,
            bind,
            type_,
            visibility,
            name,
            fmt,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let st_info = (self.bind << 4) | self.type_;
        let mut result = Vec::with_capacity(16);
        result.extend_from_slice(&self.fmt.pack_u32(self.st_name));
        result.extend_from_slice(&self.fmt.pack_u32(self.st_value));
        result.extend_from_slice(&self.fmt.pack_u32(self.st_size));
        result.push(st_info);
        result.push(self.st_other);
        result.extend_from_slice(&self.fmt.pack_u16(self.st_shndx));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSection;
    impl Section for MockSection {
        fn lookup_str(&self, _offset: u32) -> String {
            "mock_symbol".to_string()
        }
    }

    #[test]
    fn test_symbol_parse() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(1)); // st_name
        data.extend_from_slice(&fmt.pack_u32(0x1000)); // st_value
        data.extend_from_slice(&fmt.pack_u32(32)); // st_size
        data.push(0x12); // st_info (bind = 1, type = 2)
        data.push(0x3); // st_other (visibility = 3)
        data.extend_from_slice(&fmt.pack_u16(1)); // st_shndx

        let strtab = MockSection;
        let sym = Symbol::new(fmt, &data, &strtab, None).unwrap();

        assert_eq!(sym.st_name, 1);
        assert_eq!(sym.st_value, 0x1000);
        assert_eq!(sym.st_size, 32);
        assert_eq!(sym.bind, 1);
        assert_eq!(sym.type_, 2);
        assert_eq!(sym.visibility, 3);
        assert_eq!(sym.st_shndx, 1);
        assert_eq!(sym.name, "mock_symbol");
    }

    #[test]
    fn test_symbol_roundtrip() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(1)); // st_name
        data.extend_from_slice(&fmt.pack_u32(0x1000)); // st_value
        data.extend_from_slice(&fmt.pack_u32(32)); // st_size
        data.push(0x12); // st_info
        data.push(0x3); // st_other
        data.extend_from_slice(&fmt.pack_u16(1)); // st_shndx

        let strtab = MockSection;
        let sym = Symbol::new(fmt, &data, &strtab, None).unwrap();
        let bytes = sym.to_bytes();
        assert_eq!(data, bytes);
    }

    #[test]
    fn test_symbol_xindex_error() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(1)); // st_name
        data.extend_from_slice(&fmt.pack_u32(0x1000)); // st_value
        data.extend_from_slice(&fmt.pack_u32(32)); // st_size
        data.push(0x12); // st_info
        data.push(0x3); // st_other
        data.extend_from_slice(&fmt.pack_u16(SHN_XINDEX)); // st_shndx = SHN_XINDEX

        let strtab = MockSection;
        assert!(matches!(
            Symbol::new(fmt, &data, &strtab, None),
            Err(SymbolError::XindexNotSupported)
        ));
    }
}
