use crate::utils::Error;
use crate::elf::format::ElfFormat;
use crate::elf::symbol::Symbol;
use crate::elf::relocation::Relocation;
use crate::elf::constants::*;

const SHF_LINK_ORDER: u32 = 0x80;

pub trait Section {
    fn lookup_str(&self, index: usize) -> Result<String, Error>;
}

#[derive(Debug, Clone)]
pub struct ElfSection {
    pub fmt: ElfFormat,
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
    pub index: usize,
    pub name: String,
    pub relocated_by: Vec<usize>, // Indices of sections that relocate this one
}

impl ElfSection {
    pub fn new(fmt: ElfFormat, header: &[u8]) -> Result<Self, Error> {
        if header.len() < 40 {
            return Err(Error::InvalidFormat("Section header too short".into()));
        }

        let sh_name = fmt.unpack_u32(&header[0..4])?;
        let sh_type = fmt.unpack_u32(&header[4..8])?;
        let sh_flags = fmt.unpack_u32(&header[8..12])?;
        let sh_addr = fmt.unpack_u32(&header[12..16])?;
        let sh_offset = fmt.unpack_u32(&header[16..20])?;
        let sh_size = fmt.unpack_u32(&header[20..24])?;
        let sh_link = fmt.unpack_u32(&header[24..28])?;
        let sh_info = fmt.unpack_u32(&header[28..32])?;
        let sh_addralign = fmt.unpack_u32(&header[32..36])?;
        let sh_entsize = fmt.unpack_u32(&header[36..40])?;

        Ok(Self {
            fmt,
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
            index: 0,
            symbols: Vec::new(),
            relocations: Vec::new(),
            relocated_by: Vec::new(),
            name: String::new(),
        })
    }

    pub fn from_parts(fmt: ElfFormat, sh_name: u32, sh_type: u32, sh_flags: u32, sh_link: u32, 
                     sh_info: u32, sh_addralign: u32, sh_entsize: u32, data: Vec<u8>, index: usize) -> Self {
        Self {
            fmt,
            sh_name,
            sh_type,
            sh_flags,
            sh_addr: 0,
            sh_offset: 0,
            sh_size: data.len() as u32,
            sh_link,
            sh_info,
            sh_addralign,
            sh_entsize,
            data,
            symbols: Vec::new(),
            relocations: Vec::new(),
            index,
            name: String::new(),
            relocated_by: Vec::new(),
        }
    }

    pub fn is_rel(&self) -> bool {
        self.sh_type == SHT_REL || self.sh_type == SHT_RELA
    }

    pub fn header_to_bin(&self) -> Vec<u8> {
        self.fmt.pack_tuple_u32_10((
            self.sh_name,
            self.sh_type,
            self.sh_flags,
            self.sh_addr,
            self.sh_offset,
            self.sh_size,
            self.sh_link,
            self.sh_info,
            self.sh_addralign,
            self.sh_entsize
        ))
    }

    pub fn add_str(&mut self, string: &str) -> Result<u32, Error> {
        if self.sh_type != SHT_STRTAB {
            return Err(Error::InvalidSection("Not a string table section".into()));
        }
        let ret = self.data.len() as u32;
        self.data.extend_from_slice(string.as_bytes());
        self.data.push(0);
        Ok(ret)
    }

    pub fn find_symbol(&self, name: &str, sections: &[ElfSection]) -> Result<u32, Error> {
        if self.sh_type != SHT_SYMTAB {
            return Err(Error::InvalidSection("Not a symbol table section".into()));
        }

        let mut offset = 0;
        while offset + 16 <= self.data.len() {
            let relocation = Relocation::new(&self.fmt, &self.data[offset..offset + 16], self.sh_type)?;
            offset += 16;

            if relocation.sym() as usize >= sections.len() {
                continue;
            }

            let section = &sections[relocation.sym() as usize];
            if section.sh_type != SHT_STRTAB {
                continue;
            }

            if let Ok(symbol_name) = section.lookup_str(relocation.offset() as usize) {
                if symbol_name == name {
                    return Ok(relocation.offset());
                }
            }
        }

        Err(Error::SymbolNotFound(name.to_string()))
    }

    pub fn find_symbol_in_section(&self, name: &str, section: &ElfSection) -> Result<u32, Error> {
        self.find_symbol(name, &[section.clone()])
            .and_then(|offset| if offset as usize == section.index {
                Ok(offset)
            } else {
                Err(Error::InvalidSection(format!("Symbol {} not found in section", name)))
            })
    }

    pub fn local_symbols(&self) -> Result<&[Symbol], Error> {
        if self.sh_type != SHT_SYMTAB {
            return Err(Error::InvalidSection("Not a symbol table section".into()));
        }
        Ok(&self.symbols[..self.sh_info as usize])
    }

    pub fn global_symbols(&self) -> Result<&[Symbol], Error> {
        if self.sh_type != SHT_SYMTAB {
            return Err(Error::InvalidSection("Not a symbol table section".into()));
        }
        Ok(&self.symbols[self.sh_info as usize..])
    }

    pub fn late_init(&mut self, sections: &mut [ElfSection]) -> Result<(), Error> {
        if self.sh_type == SHT_REL || self.sh_type == SHT_RELA {
            let mut offset = 0;
            let entry_size = if self.sh_type == SHT_REL { 8 } else { 12 };
            while offset + entry_size <= self.data.len() {
                let relocation = Relocation::new(&self.fmt, &self.data[offset..offset + entry_size], self.sh_type)?;
                self.relocations.push(relocation);
                offset += entry_size;
            }

            // Add this section to the list of sections that relocate the target section
            if self.sh_info as usize >= sections.len() {
                return Err(Error::InvalidSection("Invalid sh_info value".into()));
            }
            sections[self.sh_info as usize].relocated_by.push(self.index);
        }
        Ok(())
    }

    pub fn relocate_mdebug(&mut self, shift_by: u32) -> Result<(), Error> {
        if self.sh_type == SHT_REL || self.sh_type == SHT_RELA {
            let mut offset = 0;
            let entry_size = if self.sh_type == SHT_REL { 8 } else { 12 };
            while offset + entry_size <= self.data.len() {
                let relocation = Relocation::new(&self.fmt, &self.data[offset..offset + entry_size], self.sh_type)?;
                offset += entry_size;

                // Only relocate mdebug sections
                if relocation.type_() != 0x7d {
                    continue;
                }

                // Apply relocation
                let target_offset = relocation.offset() as usize;
                if target_offset + 4 > self.data.len() {
                    return Err(Error::InvalidRelocation("Relocation offset out of bounds".into()));
                }

                let current_value = self.fmt.unpack_u32(&self.data[target_offset..target_offset + 4])?;
                let new_value = current_value + shift_by;
                let new_bytes = self.fmt.pack_u32(new_value);
                self.data[target_offset..target_offset + 4].copy_from_slice(&new_bytes);
            }
        }
        Ok(())
    }

    pub fn init_data(&mut self, data: &[u8]) -> Result<(), Error> {
        self.data = data[self.sh_offset as usize..(self.sh_offset + self.sh_size) as usize].to_vec();
        Ok(())
    }

    pub fn init_symbols(&mut self) -> Result<(), Error> {
        let mut symbols = Vec::new();
        let mut offset = 0;
        while offset < self.data.len() {
            let symbol = Symbol::new(&self.fmt, &self.data[offset..offset + 16])?;
            symbols.push(symbol);
            offset += 16;
        }
        self.symbols = symbols;
        Ok(())
    }

    pub fn init_relocations(&mut self) -> Result<(), Error> {
        if self.sh_type != SHT_REL && self.sh_type != SHT_RELA {
            return Ok(());
        }

        let mut offset = 0;
        let entry_size = if self.sh_type == SHT_REL { 8 } else { 12 };
        while offset + entry_size <= self.data.len() {
            let relocation = Relocation::new(&self.fmt, &self.data[offset..offset + entry_size], self.sh_type)?;
            self.relocations.push(relocation);
            offset += entry_size;
        }
        Ok(())
    }
}

impl Section for ElfSection {
    fn lookup_str(&self, index: usize) -> Result<String, Error> {
        if self.sh_type != SHT_STRTAB {
            return Err(Error::InvalidSection("Not a string table section".into()));
        }

        let end = self.data[index..]
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| Error::InvalidSection("String not null-terminated".into()))?;

        String::from_utf8(self.data[index..index + end].to_vec())
            .map_err(|_| Error::InvalidSection("Invalid UTF-8 in string table".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_header() {
        let fmt = ElfFormat::new(true);
        let mut data = Vec::new();
        data.extend_from_slice(&fmt.pack_u32(1)); // sh_name
        data.extend_from_slice(&fmt.pack_u32(2)); // sh_type
        data.extend_from_slice(&fmt.pack_u32(3)); // sh_flags
        data.extend_from_slice(&fmt.pack_u32(4)); // sh_addr
        data.extend_from_slice(&fmt.pack_u32(5)); // sh_offset
        data.extend_from_slice(&fmt.pack_u32(6)); // sh_size
        data.extend_from_slice(&fmt.pack_u32(7)); // sh_link
        data.extend_from_slice(&fmt.pack_u32(8)); // sh_info
        data.extend_from_slice(&fmt.pack_u32(9)); // sh_addralign
        data.extend_from_slice(&fmt.pack_u32(10)); // sh_entsize

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

        let packed = section.header_to_bin();
        assert_eq!(data, packed);
    }

    #[test]
    fn test_section_data() {
        let fmt = ElfFormat::new(true);
        let mut section = ElfSection::new(fmt, &vec![0; 40]).unwrap();
        section.sh_offset = 10;
        section.sh_size = 5;

        let file_data = b"0123456789ABCDEF".to_vec();
        section.init_data(&file_data).unwrap();

        assert_eq!(section.data, b"56789");
    }

    #[test]
    fn test_string_table() {
        let fmt = ElfFormat::new(true);
        let mut section = ElfSection::new(fmt, &vec![0; 40]).unwrap();
        section.sh_type = SHT_STRTAB;

        let string_data = b"test\0string\0".to_vec();
        section.data = string_data;

        assert_eq!(section.lookup_str(0).unwrap(), "test");
        assert_eq!(section.lookup_str(5).unwrap(), "string");
    }

    #[test]
    fn test_from_parts() {
        let fmt = ElfFormat::new(true);
        let data = b"test data".to_vec();
        let section = ElfSection::from_parts(
            fmt,
            1, // sh_name
            SHT_PROGBITS, // sh_type
            0, // sh_flags
            0, // sh_link
            0, // sh_info
            4, // sh_addralign
            0, // sh_entsize
            data.clone(),
            5, // index
        );

        assert_eq!(section.sh_name, 1);
        assert_eq!(section.sh_type, SHT_PROGBITS);
        assert_eq!(section.sh_size, data.len() as u32);
        assert_eq!(section.index, 5);
        assert_eq!(section.data, data);
    }

    #[test]
    fn test_add_str() {
        let fmt = ElfFormat::new(true);
        let mut section = ElfSection::from_parts(
            fmt,
            0,
            SHT_STRTAB,
            0,
            0,
            0,
            1,
            0,
            vec![],
            1,
        );

        let pos1 = section.add_str("test1").unwrap();
        let pos2 = section.add_str("test2").unwrap();

        assert_eq!(pos1, 0);
        assert_eq!(pos2, 6); // "test1\0" is 6 bytes
        assert_eq!(section.lookup_str(0).unwrap(), "test1");
        assert_eq!(section.lookup_str(6).unwrap(), "test2");
    }

    #[test]
    fn test_symbol_operations() {
        let fmt = ElfFormat::new(true);
        let mut section = ElfSection::from_parts(
            fmt,
            0,
            SHT_SYMTAB,
            0,
            0,
            0,
            1,
            2, // First 2 symbols are local
            vec![],
            1,
        );

        // Add some test symbols
        let mut sym1 = Symbol::default();
        sym1.st_name = 0;
        sym1.st_value = 100;
        sym1.st_shndx = 1;
        sym1.name = "local1".to_string();

        let mut sym2 = Symbol::default();
        sym2.st_name = 6;
        sym2.st_value = 200;
        sym2.st_shndx = 1;
        sym2.name = "local2".to_string();

        let mut sym3 = Symbol::default();
        sym3.st_name = 12;
        sym3.st_value = 300;
        sym3.st_shndx = 2;
        sym3.name = "global1".to_string();

        section.symbols = vec![sym1.clone(), sym2.clone(), sym3.clone()];

        // Test symbol lookup
        let found = section.find_symbol("local1", &[]).unwrap();
        assert_eq!(found, 100);

        // Test local/global symbol separation
        let locals = section.local_symbols().unwrap();
        let globals = section.global_symbols().unwrap();
        assert_eq!(locals.len(), 2);
        assert_eq!(globals.len(), 1);
        assert_eq!(locals[0].name, "local1");
        assert_eq!(globals[0].name, "global1");

        // Test find_symbol_in_section
        let dummy_section = ElfSection::from_parts(
            fmt,
            0,
            SHT_PROGBITS,
            0,
            0,
            0,
            0,
            0,
            vec![],
            1,
        );
        let value = section.find_symbol_in_section("local1", &dummy_section).unwrap();
        assert_eq!(value, 100);
    }

    #[test]
    fn test_late_init() {
        let fmt = ElfFormat { is_big_endian: false };
        let mut sections = vec![
            ElfSection {
                fmt,
                sh_name: 0,
                sh_type: SHT_PROGBITS,
                sh_flags: 0,
                sh_addr: 0,
                sh_offset: 0,
                sh_size: 0,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 0,
                sh_entsize: 0,
                data: Vec::new(),
                index: 0,
                name: String::new(),
                symbols: Vec::new(),
                relocations: Vec::new(),
                relocated_by: Vec::new(),
            },
            ElfSection {
                fmt,
                sh_name: 0,
                sh_type: SHT_REL,
                sh_flags: 0,
                sh_addr: 0,
                sh_offset: 0,
                sh_size: 0,
                sh_link: 0,
                sh_info: 0, // Points to section 0
                sh_addralign: 0,
                sh_entsize: 0,
                data: Vec::new(),
                index: 1,
                name: String::new(),
                symbols: Vec::new(),
                relocations: Vec::new(),
                relocated_by: Vec::new(),
            },
        ];
        
        // Initialize relocations
        let (target, rest) = sections.split_at_mut(1);
        rest[0].late_init(target).unwrap();
        
        // Check that target section is marked as being relocated
        assert!(sections[0].relocated_by.contains(&1));
    }

    #[test]
    fn test_mdebug_relocation() {
        let fmt = ElfFormat::new(true);
        let mut section = ElfSection::from_parts(
            fmt,
            0,
            SHT_MIPS_DEBUG,
            0,
            0,
            0,
            1,
            0,
            vec![0; 0x60], // Minimum size for HDRR
            1,
        );

        // Set up a dummy HDRR structure
        let mut hdrr_data = vec![];
        hdrr_data.extend_from_slice(&fmt.pack_u16(0x7009)); // magic
        hdrr_data.extend_from_slice(&fmt.pack_u16(0)); // vstamp
        for _ in 0..23 {
            hdrr_data.extend_from_slice(&fmt.pack_u32(100)); // Various offsets
        }
        section.data = hdrr_data;

        // Test relocation
        section.sh_offset = 1000;
        section.relocate_mdebug(500).unwrap();

        // Verify that offsets were updated
        let new_hdrr = section.fmt.unpack_tuple_u32_25(&section.data[4..]).unwrap();
        assert_eq!(new_hdrr.0, 600); // First offset should be shifted by 500
    }
}
