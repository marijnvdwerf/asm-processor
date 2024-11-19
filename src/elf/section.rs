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

    pub fn header_to_bin(&self) -> Result<Vec<u8>, Error> {
        let mut result = vec![0; 40];
        self.fmt.pack_u32(&mut result[0..4], self.sh_name)?;
        self.fmt.pack_u32(&mut result[4..8], self.sh_type)?;
        self.fmt.pack_u32(&mut result[8..12], self.sh_flags)?;
        self.fmt.pack_u32(&mut result[12..16], self.sh_addr)?;
        self.fmt.pack_u32(&mut result[16..20], self.sh_offset)?;
        self.fmt.pack_u32(&mut result[20..24], self.sh_size)?;
        self.fmt.pack_u32(&mut result[24..28], self.sh_link)?;
        self.fmt.pack_u32(&mut result[28..32], self.sh_info)?;
        self.fmt.pack_u32(&mut result[32..36], self.sh_addralign)?;
        self.fmt.pack_u32(&mut result[36..40], self.sh_entsize)?;
        Ok(result)
    }

    pub fn header_to_test_data(&self) -> Vec<u8> {
        let mut data = vec![0; 40];
        let fmt = ElfFormat::new(true);
        
        let mut tmp = [0; 4];
        fmt.pack_u32(&mut tmp, 1).unwrap();
        data[0..4].copy_from_slice(&tmp);
        
        fmt.pack_u32(&mut tmp, 2).unwrap();
        data[4..8].copy_from_slice(&tmp);
        
        fmt.pack_u32(&mut tmp, 3).unwrap();
        data[8..12].copy_from_slice(&tmp);
        
        fmt.pack_u32(&mut tmp, 4).unwrap();
        data[12..16].copy_from_slice(&tmp);
        
        fmt.pack_u32(&mut tmp, 5).unwrap();
        data[16..20].copy_from_slice(&tmp);
        
        fmt.pack_u32(&mut tmp, 6).unwrap();
        data[20..24].copy_from_slice(&tmp);
        
        fmt.pack_u32(&mut tmp, 7).unwrap();
        data[24..28].copy_from_slice(&tmp);
        
        fmt.pack_u32(&mut tmp, 8).unwrap();
        data[28..32].copy_from_slice(&tmp);
        
        fmt.pack_u32(&mut tmp, 9).unwrap();
        data[32..36].copy_from_slice(&tmp);
        
        fmt.pack_u32(&mut tmp, 10).unwrap();
        data[36..40].copy_from_slice(&tmp);
        
        data
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

        for symbol in &self.symbols {
            if symbol.name == name {
                return Ok(symbol.st_value);
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

    pub fn relocate_mdebug(&mut self, original_offset: u32) -> Result<(), Error> {
        if self.sh_type != SHT_MIPS_DEBUG {
            return Err(Error::InvalidFormat("Not a MIPS_DEBUG section".to_string()));
        }

        let mut new_data = self.data.clone();
        let shift_by = self.sh_offset - original_offset;

        // First unpack the magic and version stamp as u16s
        let magic = self.fmt.unpack_u16(&self.data[0..2])?;
        let vstamp = self.fmt.unpack_u16(&self.data[2..4])?;

        if magic != 0x7009 {
            return Err(Error::InvalidFormat("Invalid magic value for .mdebug symbolic header".to_string()));
        }

        // Now unpack the remaining 23 u32s
        let mut values = self.fmt.unpack_tuple_u32(&self.data[4..0x60], 23)?;
        
        // Update offsets if count is non-zero
        // ilineMax, cbLine, cbLineOffset
        if values[0] > 0 { values[2] += shift_by; }
        // idnMax, cbDnOffset
        if values[3] > 0 { values[4] += shift_by; }
        // ipdMax, cbPdOffset
        if values[5] > 0 { values[6] += shift_by; }
        // isymMax, cbSymOffset
        if values[7] > 0 { values[8] += shift_by; }
        // ioptMax, cbOptOffset
        if values[9] > 0 { values[10] += shift_by; }
        // iauxMax, cbAuxOffset
        if values[11] > 0 { values[12] += shift_by; }
        // issMax, cbSsOffset
        if values[13] > 0 { values[14] += shift_by; }
        // issExtMax, cbSsExtOffset
        if values[15] > 0 { values[16] += shift_by; }
        // ifdMax, cbFdOffset
        if values[17] > 0 { values[18] += shift_by; }
        // crfd, cbRfdOffset
        if values[19] > 0 { values[20] += shift_by; }
        // iextMax, cbExtOffset
        if values[21] > 0 { values[22] += shift_by; }

        // Pack magic and vstamp back
        self.fmt.pack_u16(&mut new_data[0..2], magic)?;
        self.fmt.pack_u16(&mut new_data[2..4], vstamp)?;

        // Pack the updated values back
        self.fmt.pack_tuple_u32(&mut new_data[4..0x60], &values)?;

        self.data = new_data;
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

        let packed = section.header_to_bin().unwrap();
        assert_eq!(data, packed);
    }

    #[test]
    fn test_section_data() {
        let fmt = ElfFormat { is_big_endian: false };
        let mut section = ElfSection {
            fmt,
            sh_name: 0,
            sh_type: SHT_PROGBITS,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: 0,
            sh_size: 5,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 0,
            sh_entsize: 0,
            data: vec![65, 66, 67, 68, 69], // "ABCDE"
            index: 0,
            name: String::new(),
            symbols: Vec::new(),
            relocations: Vec::new(),
            relocated_by: Vec::new(),
        };

        // Test data access
        assert_eq!(section.data, [65, 66, 67, 68, 69]);

        // Test data modification
        section.data = vec![53, 54, 55, 56, 57]; // "12345"
        assert_eq!(section.data, [53, 54, 55, 56, 57]);
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
        let fmt = ElfFormat { is_big_endian: false };
        let mut section = ElfSection {
            fmt,
            sh_name: 0,
            sh_type: SHT_SYMTAB,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: 0,
            sh_size: 16,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 0,
            sh_entsize: 16,
            data: vec![],
            index: 0,
            name: String::new(),
            symbols: vec![
                Symbol {
                    st_name: 1,
                    st_value: 2,
                    st_size: 4,
                    st_info: 0,
                    st_other: 0,
                    st_shndx: 1,
                    name: "local1".to_string(),
                }
            ],
            relocations: Vec::new(),
            relocated_by: Vec::new(),
        };

        let sections = vec![section.clone()];
        assert_eq!(section.find_symbol("local1", &sections).unwrap(), 2);
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
        let fmt = ElfFormat { is_big_endian: false };
        let mut section = ElfSection {
            fmt,
            sh_name: 0,
            sh_type: SHT_MIPS_DEBUG,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: 0x2000,
            sh_size: 100,  // 25 * 4 bytes
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 0,
            sh_entsize: 0,
            data: {
                let mut data = vec![0; 100];  // 25 * 4 bytes
                // Set magic value (0x7009) and some test values
                data[0] = 0x09;
                data[1] = 0x70;
                // Set a count and offset to test relocation
                data[8] = 1;  // ilinemax = 1
                data[16] = 0x10; // cblineoff = 0x10
                data
            },
            index: 0,
            name: String::new(),
            symbols: Vec::new(),
            relocations: Vec::new(),
            relocated_by: Vec::new(),
        };

        // Apply relocation
        section.relocate_mdebug(0x1000).unwrap();

        // Check that the offset was updated
        let offset = section.fmt.unpack_u32(&section.data[16..20]).unwrap();
        assert_eq!(offset, 0x10 + (0x2000 - 0x1000));
    }
}
