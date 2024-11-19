use crate::utils::Error;
use crate::elf::format::ElfFormat;
use crate::elf::symbol::Symbol;
use crate::elf::relocation::Relocation;
use crate::elf::constants::*;

pub trait Section {
    fn lookup_str(&self, index: usize) -> Result<String, Error>;
}

#[derive(Debug, Clone)]
pub struct ElfSection {
    pub sh_name: u32,
    pub sh_type: u32,
    pub sh_flags: u32,
    pub sh_addr: u32,
    pub sh_offset: u32,
    pub sh_size: u32,
    pub sh_link: u32,
    pub sh_info: u32,
    pub sh_addralign: u32,
    pub sh_entsize: u32,
    pub data: Vec<u8>,
    pub symbols: Vec<Symbol>,
    pub relocations: Vec<Relocation>,
    fmt: ElfFormat,
}

impl ElfSection {
    pub fn new(fmt: ElfFormat, data: &[u8]) -> Result<Self, Error> {
        let (sh_name, sh_type, sh_flags, sh_addr, sh_offset, sh_size, sh_link, sh_info, sh_addralign, sh_entsize) = 
            fmt.unpack_tuple_u32_10(data)?;

        Ok(Self {
            sh_name,
            sh_type,
            sh_flags,
            sh_addr,
            sh_offset,
            sh_size,
            sh_link,
            sh_info,
            sh_addralign,
            sh_entsize,
            data: Vec::new(),
            symbols: Vec::new(),
            relocations: Vec::new(),
            fmt,
        })
    }

    pub fn init_data(&mut self, file_data: &[u8]) -> Result<(), Error> {
        if self.sh_type == SHT_NOBITS {
            self.data = vec![0; self.sh_size as usize];
        } else {
            let start = self.sh_offset as usize;
            let end = start + self.sh_size as usize;
            if end > file_data.len() {
                return Err(Error::InvalidSection("Section data extends beyond file".into()));
            }
            self.data = file_data[start..end].to_vec();
        }
        Ok(())
    }

    pub fn init_symbols(&mut self) -> Result<(), Error> {
        if self.sh_type != SHT_SYMTAB && self.sh_type != SHT_DYNSYM {
            return Ok(());
        }

        if self.sh_entsize == 0 {
            return Err(Error::InvalidSection("Symbol entry size is 0".into()));
        }

        let num_entries = self.sh_size / self.sh_entsize;
        self.symbols.clear();

        for i in 0..num_entries {
            let start = (i * self.sh_entsize) as usize;
            let end = start + self.sh_entsize as usize;
            if end > self.data.len() {
                return Err(Error::InvalidSection("Symbol data extends beyond section".into()));
            }
            let symbol = Symbol::new(&self.fmt, &self.data[start..end])?;
            self.symbols.push(symbol);
        }

        Ok(())
    }

    pub fn init_relocations(&mut self) -> Result<(), Error> {
        if self.sh_type != SHT_REL && self.sh_type != SHT_RELA {
            return Ok(());
        }

        if self.sh_entsize == 0 {
            return Err(Error::InvalidSection("Relocation entry size is 0".into()));
        }

        let num_entries = self.sh_size / self.sh_entsize;
        self.relocations.clear();

        for i in 0..num_entries {
            let start = (i * self.sh_entsize) as usize;
            let end = start + self.sh_entsize as usize;
            if end > self.data.len() {
                return Err(Error::InvalidSection("Relocation data extends beyond section".into()));
            }
            let reloc = Relocation::new(&self.fmt, &self.data[start..end], self.sh_type)?;
            self.relocations.push(reloc);
        }

        Ok(())
    }

    pub fn pack_header(&self) -> Vec<u8> {
        self.fmt.pack_tuple_u32_10(
            self.sh_name,
            self.sh_type,
            self.sh_flags,
            self.sh_addr,
            self.sh_offset,
            self.sh_size,
            self.sh_link,
            self.sh_info,
            self.sh_addralign,
            self.sh_entsize,
        )
    }
}

impl Section for ElfSection {
    fn lookup_str(&self, index: usize) -> Result<String, Error> {
        if self.sh_type != SHT_STRTAB {
            return Err(Error::InvalidSection("Not a string table".into()));
        }

        if index >= self.data.len() {
            return Err(Error::InvalidSection("String index out of bounds".into()));
        }

        let mut end = index;
        while end < self.data.len() && self.data[end] != 0 {
            end += 1;
        }

        if end >= self.data.len() {
            return Err(Error::InvalidSection("Unterminated string".into()));
        }

        String::from_utf8(self.data[index..end].to_vec())
            .map_err(|_| Error::InvalidSection("Invalid UTF-8 string".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_header() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        for i in 1..=10 {
            data.extend_from_slice(&fmt.pack_u32(i));
        }

        let section = ElfSection::new(fmt, &data).unwrap();
        assert_eq!(section.sh_name, 1);
        assert_eq!(section.sh_type, 2);
        assert_eq!(section.sh_flags, 3);
        assert_eq!(section.sh_addr, 4);
        assert_eq!(section.sh_offset, 5);
        assert_eq!(section.sh_size, 6);
        assert_eq!(section.sh_link, 7);
        assert_eq!(section.sh_info, 8);
        assert_eq!(section.sh_addralign, 9);
        assert_eq!(section.sh_entsize, 10);

        let packed = section.pack_header();
        assert_eq!(data, packed);
    }

    #[test]
    fn test_section_data() {
        let fmt = ElfFormat::new(true);
        let mut header_data = Vec::new();
        for i in 1..=10 {
            header_data.extend_from_slice(&fmt.pack_u32(i));
        }

        let mut section = ElfSection::new(fmt, &header_data).unwrap();
        let file_data = vec![0; 100];
        section.init_data(&file_data).unwrap();

        assert_eq!(section.data.len(), section.sh_size as usize);
    }

    #[test]
    fn test_string_table() {
        let fmt = ElfFormat::new(true);
        let mut header_data = Vec::new();
        header_data.extend_from_slice(&fmt.pack_u32(0)); // sh_name
        header_data.extend_from_slice(&fmt.pack_u32(SHT_STRTAB)); // sh_type
        for i in 3..=10 {
            header_data.extend_from_slice(&fmt.pack_u32(i));
        }

        let mut section = ElfSection::new(fmt, &header_data).unwrap();
        let mut string_data = Vec::new();
        string_data.extend_from_slice(b"test\0string\0");
        section.data = string_data;

        assert_eq!(section.lookup_str(0).unwrap(), "test");
        assert_eq!(section.lookup_str(5).unwrap(), "string");
    }
}
