use crate::utils::Error;
use crate::elf::format::ElfFormat;
use crate::elf::symbol::Symbol;
use crate::elf::relocation::Relocation;
use crate::elf::constants::*;

const SHF_LINK_ORDER: u32 = 0x80;

pub trait Section {
    fn lookup_str(&self, index: usize) -> Result<String, Error>;
    fn add_str(&mut self, s: &str) -> Result<u32, Error>;
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
            index,
            symbols: Vec::new(),
            relocations: Vec::new(),
            name: String::new(),
            relocated_by: Vec::new(),
        }
    }

    pub fn is_rel(&self) -> bool {
        self.sh_type == SHT_REL || self.sh_type == SHT_RELA
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = vec![0; 40];
        let fmt = ElfFormat::new(true);
        
        // Update sh_size to match data length if needed
        let size = if self.sh_type != SHT_NOBITS && !self.data.is_empty() {
            self.data.len() as u32
        } else {
            self.sh_size
        };
        
        let mut tmp = [0; 4];
        
        // sh_name
        fmt.pack_u32(&mut tmp, self.sh_name).unwrap();
        data[0..4].copy_from_slice(&tmp);
        
        // sh_type
        fmt.pack_u32(&mut tmp, self.sh_type).unwrap();
        data[4..8].copy_from_slice(&tmp);
        
        // sh_flags
        fmt.pack_u32(&mut tmp, self.sh_flags).unwrap();
        data[8..12].copy_from_slice(&tmp);
        
        // sh_addr
        fmt.pack_u32(&mut tmp, self.sh_addr).unwrap();
        data[12..16].copy_from_slice(&tmp);
        
        // sh_offset
        fmt.pack_u32(&mut tmp, self.sh_offset).unwrap();
        data[16..20].copy_from_slice(&tmp);
        
        // sh_size
        fmt.pack_u32(&mut tmp, size).unwrap();
        data[20..24].copy_from_slice(&tmp);
        
        // sh_link
        fmt.pack_u32(&mut tmp, self.sh_link).unwrap();
        data[24..28].copy_from_slice(&tmp);
        
        // sh_info
        fmt.pack_u32(&mut tmp, self.sh_info).unwrap();
        data[28..32].copy_from_slice(&tmp);
        
        // sh_addralign
        fmt.pack_u32(&mut tmp, self.sh_addralign).unwrap();
        data[32..36].copy_from_slice(&tmp);
        
        // sh_entsize
        fmt.pack_u32(&mut tmp, self.sh_entsize).unwrap();
        data[36..40].copy_from_slice(&tmp);
        
        data
    }

    pub fn to_test_data(&self) -> Vec<u8> {
        let mut data = vec![0; 40];
        let fmt = ElfFormat::new(true);
        
        let mut tmp = [0; 4];
        
        // Pack test values 1-10 into the buffer
        for i in 0..10 {
            fmt.pack_u32(&mut tmp, i as u32 + 1).unwrap();
            let start = i * 4;
            data[start..start+4].copy_from_slice(&tmp);
        }
        
        data
    }

    pub fn add_str(&mut self, string: &str) -> Result<u32, Error> {
        if self.sh_type != SHT_STRTAB {
            return Err(Error::InvalidSection("Not a string table section".into()));
        }
        let ret = self.data.len() as u32;
        // Convert to latin1 bytes like Python
        self.data.extend(string.chars().map(|c| c as u8));
        self.data.push(0);
        self.sh_size = self.data.len() as u32;  // Update sh_size to match data length
        Ok(ret)
    }

    pub fn find_symbol(&self, name: &str, sections: &[ElfSection]) -> Result<Option<(usize, u32)>, Error> {
        if self.sh_type != SHT_SYMTAB {
            return Err(Error::InvalidSection("Not a symbol table section".into()));
        }
        for symbol in &self.symbols {
            if symbol.name == name {
                return Ok(Some((symbol.st_shndx as usize, symbol.st_value)));
            }
        }
        Ok(None)
    }

    pub fn find_symbol_in_section(&self, name: &str, section: &ElfSection) -> Result<u32, Error> {
        let pos = self.find_symbol(name, &[section.clone()])?
            .ok_or_else(|| Error::InvalidSection("Symbol not found".into()))?;
        if pos.0 != section.index {
            return Err(Error::InvalidSection("Symbol not in specified section".into()));
        }
        Ok(pos.1)
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
        if self.sh_type == SHT_SYMTAB {
            self.init_symbols()?;
        } else if self.is_rel() {
            let mut offset = 0;
            let entry_size = if self.sh_type == SHT_REL { 8 } else { 12 };
            while offset + entry_size <= self.data.len() {
                let relocation = Relocation::new(&self.fmt, &self.data[offset..offset + entry_size], self.sh_type)?;
                self.relocations.push(relocation);
                offset += entry_size;
            }

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
        let shift_by = self.sh_offset.wrapping_sub(original_offset);

        // First unpack the magic and version stamp
        let magic = self.fmt.unpack_u16(&self.data[0..2])?;
        let vstamp = self.fmt.unpack_u16(&self.data[2..4])?;

        if magic != 0x7009 {
            return Err(Error::InvalidFormat("Invalid magic value for .mdebug symbolic header".to_string()));
        }

        // Now unpack the remaining values
        let mut values = self.fmt.unpack_tuple_u32(&self.data[4..0x60], 23)?;

        // Update offsets if count is non-zero (matching Python implementation)
        if values[0] > 0 { values[2] = values[2].wrapping_add(shift_by); }  // ilineMax -> cbLineOffset
        if values[3] > 0 { values[4] = values[4].wrapping_add(shift_by); }  // idnMax -> cbDnOffset
        if values[5] > 0 { values[6] = values[6].wrapping_add(shift_by); }  // ipdMax -> cbPdOffset
        if values[7] > 0 { values[8] = values[8].wrapping_add(shift_by); }  // isymMax -> cbSymOffset
        if values[9] > 0 { values[10] = values[10].wrapping_add(shift_by); }  // ioptMax -> cbOptOffset
        if values[11] > 0 { values[12] = values[12].wrapping_add(shift_by); }  // iauxMax -> cbAuxOffset
        if values[13] > 0 { values[14] = values[14].wrapping_add(shift_by); }  // issMax -> cbSsOffset
        if values[15] > 0 { values[16] = values[16].wrapping_add(shift_by); }  // issExtMax -> cbSsExtOffset
        if values[17] > 0 { values[18] = values[18].wrapping_add(shift_by); }  // ifdMax -> cbFdOffset
        if values[19] > 0 { values[20] = values[20].wrapping_add(shift_by); }  // crfd -> cbRfdOffset
        if values[21] > 0 { values[22] = values[22].wrapping_add(shift_by); }  // iextMax -> cbExtOffset

        // Pack everything back
        self.fmt.pack_u16(&mut new_data[0..2], magic)?;
        self.fmt.pack_u16(&mut new_data[2..4], vstamp)?;
        self.fmt.pack_tuple_u32(&mut new_data[4..0x60], &values)?;

        self.data = new_data;
        Ok(())
    }

    pub fn init_data(&mut self, data: &[u8]) -> Result<(), Error> {
        self.data = data[self.sh_offset as usize..(self.sh_offset + self.sh_size) as usize].to_vec();
        Ok(())
    }

    pub fn init_symbols(&mut self) -> Result<(), Error> {
        if self.sh_type != SHT_SYMTAB {
            return Ok(());
        }

        let mut symbols = Vec::new();
        let mut offset = 0;
        while offset + 16 <= self.data.len() {
            let symbol = Symbol::new(&self.fmt, &self.data[offset..offset + 16], self)?;
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

    pub fn symbol_entries(&self) -> Result<&[Symbol], Error> {
        if self.sh_type != SHT_SYMTAB {
            return Err(Error::InvalidSection("Not a symbol table section".into()));
        }
        Ok(&self.symbols)
    }
}

impl Default for ElfSection {
    fn default() -> Self {
        Self {
            fmt: ElfFormat::new(true),
            sh_name: 0,
            sh_type: 0,
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
            symbols: Vec::new(),
            relocations: Vec::new(),
            name: String::new(),
            relocated_by: Vec::new(),
        }
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

        // Use latin1 encoding like Python
        Ok(self.data[index..index + end].iter().map(|&b| b as char).collect())
    }

    fn add_str(&mut self, s: &str) -> Result<u32, Error> {
        if self.sh_type != SHT_STRTAB {
            return Err(Error::InvalidSection("Not a string table section".into()));
        }
        let ret = self.data.len() as u32;
        // Convert to latin1 bytes like Python
        self.data.extend(s.chars().map(|c| c as u8));
        self.data.push(0);
        self.sh_size = self.data.len() as u32;
        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_header() {
        let data = {
            let mut section = ElfSection::default();
            section.to_test_data()
        };

        let mut section = ElfSection::default();
        section.sh_name = 1;
        section.sh_type = 2;
        section.sh_flags = 3;
        section.sh_addr = 4;
        section.sh_offset = 5;
        section.sh_size = 6;
        section.sh_link = 7;
        section.sh_info = 8;
        section.sh_addralign = 9;
        section.sh_entsize = 10;

        let packed = section.to_bytes();
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
        assert_eq!(section.find_symbol("local1", &sections).unwrap(), Some((1, 2)));
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
        let mut section = ElfSection {
            fmt,
            sh_name: 0,
            sh_type: SHT_MIPS_DEBUG,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: 0x2000,  // Set offset to 0x2000
            sh_size: 0,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 0,
            sh_entsize: 0,
            data: {
                let mut data = vec![0; 0x60];  // Initialize with enough space
                
                // Pack the magic value (0x7009) and version stamp (1)
                fmt.pack_u16(&mut data[0..2], 0x7009).unwrap();
                fmt.pack_u16(&mut data[2..4], 1).unwrap();
                
                // Initialize all values to 0
                let mut values = vec![0u32; 23];
                
                // Set test values: ilineMax = 1, cbLineOffset = 0x10
                values[0] = 1;  // ilineMax
                values[2] = 0x10;  // cbLineOffset
                
                // Pack the values
                fmt.pack_tuple_u32(&mut data[4..0x60], &values).unwrap();
                
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

        // Check that the offset was updated correctly
        let values = section.fmt.unpack_tuple_u32(&section.data[4..0x60], 23).unwrap();
        assert_eq!(values[2], 0x1010);  // 0x10 + (0x2000 - 0x1000)
    }
}
