use std::fs::File;
use std::io::{Write, Seek, SeekFrom};
use crate::utils::Error;
use crate::elf::format::ElfFormat;
use crate::elf::header::ElfHeader;
use crate::elf::section::{ElfSection, Section};
use crate::elf::constants::*;

#[derive(Debug)]
pub struct ElfFile {
    pub header: ElfHeader,
    pub sections: Vec<ElfSection>,
    pub fmt: ElfFormat,
    pub symtab: usize,  // Index of symbol table section
}

impl ElfFile {
    pub fn new(data: &[u8]) -> Result<Self, Error> {
        // Check ELF magic
        if data.len() < 4 || &data[0..4] != b"\x7fELF" {
            return Err(Error::InvalidFormat("Not an ELF file".into()));
        }

        // Create format and parse header
        let fmt = ElfFormat::new(data[EI_DATA] == 2); // EI_DATA == 2 means big endian
        let header = ElfHeader::new(&fmt, &data[0..52])?;

        // Parse sections
        let mut sections = Vec::new();
        let offset = header.e_shoff as usize;
        let size = header.e_shentsize as usize;

        // Parse null section first
        let mut null_section = ElfSection::new(fmt, &data[offset..offset + size])?;
        let num_sections = if header.e_shnum == 0 {
            null_section.sh_size
        } else {
            header.e_shnum as u32
        };

        null_section.index = 0;
        sections.push(null_section);

        // Parse remaining sections
        for i in 1..num_sections {
            let ind = offset + (i as usize) * size;
            let mut section = ElfSection::new(fmt, &data[ind..ind + size])?;
            section.index = i as usize;
            section.init_data(data)?;
            sections.push(section);
        }

        // Find symbol table section
        let mut symtab = None;
        for (i, s) in sections.iter().enumerate() {
            if s.sh_type == SHT_SYMTAB {
                if symtab.is_some() {
                    return Err(Error::InvalidFormat("Multiple symbol tables found".into()));
                }
                symtab = Some(i);
            }
        }

        let symtab = symtab.ok_or_else(|| Error::InvalidFormat("No symbol table found".into()))?;

        let mut file = Self {
            header,
            sections,
            fmt,
            symtab,
        };

        // Initialize section names and perform late initialization
        let shstr_idx = file.header.e_shstrndx as usize;
        for i in 0..file.sections.len() {
            let name = file.sections[shstr_idx].lookup_str(file.sections[i].sh_name as usize)?;
            file.sections[i].name = name;
        }

        // Late init sections
        for i in 0..file.sections.len() {
            let mut sections_clone = file.sections.clone();
            file.sections[i].late_init(&mut sections_clone)?;
        }

        Ok(file)
    }

    pub fn find_section(&self, name: &str) -> Option<&ElfSection> {
        self.sections.iter().find(|s| s.name == name)
    }

    pub fn add_section(&mut self, name: &str, sh_type: u32, sh_flags: u32, 
                      sh_link: u32, sh_info: u32, sh_addralign: u32, 
                      sh_entsize: u32, data: Vec<u8>) -> Result<usize, Error> {
        let shstr = &mut self.sections[self.header.e_shstrndx as usize];
        let sh_name = shstr.add_str(name)?;
        
        let index = self.sections.len();
        let section = ElfSection::from_parts(
            self.fmt,
            sh_name,
            sh_type,
            sh_flags,
            sh_link,
            sh_info,
            sh_addralign,
            sh_entsize,
            data,
            index
        );

        self.sections.push(section);
        let mut sections_clone = self.sections.clone();
        self.sections[index].late_init(&mut sections_clone)?;
        
        Ok(index)
    }

    pub fn drop_mdebug_gptab(&mut self) {
        while let Some(section) = self.sections.last() {
            if section.sh_type != SHT_MIPS_DEBUG && section.sh_type != SHT_MIPS_GPTAB {
                break;
            }
            self.sections.pop();
        }
    }

    pub fn write(&mut self, filename: &str) -> Result<(), Error> {
        let mut file = File::create(filename)?;
        let mut outidx: u32 = 0;

        // Write header
        self.header.e_shnum = self.sections.len() as u16;
        let header_bytes = self.header.to_bytes(&self.fmt)?;
        file.write_all(&header_bytes)?;
        outidx += header_bytes.len() as u32;

        // Write section data
        for section in self.sections.iter_mut() {
            if section.sh_type != SHT_NOBITS && section.sh_type != SHT_NULL {
                // Pad to alignment
                if section.sh_addralign > 0 && outidx % section.sh_addralign != 0 {
                    let padding = section.sh_addralign - (outidx % section.sh_addralign);
                    let padding_bytes = vec![0; padding as usize];
                    file.write_all(&padding_bytes)?;
                    outidx += padding as u32;
                }

                let old_offset = section.sh_offset;
                section.sh_offset = outidx;
                
                if section.sh_type == SHT_MIPS_REGINFO && section.sh_offset != old_offset {
                    section.relocate_mdebug(old_offset)?;
                }
                
                file.write_all(&section.data)?;
                outidx += section.data.len() as u32;
            }
        }

        // Pad to 4-byte alignment for section headers
        if outidx % 4 != 0 {
            let padding = 4 - (outidx % 4);
            let padding_bytes = vec![0; padding as usize];
            file.write_all(&padding_bytes)?;
            outidx += padding;
        }

        // Write section headers
        self.header.e_shoff = outidx;
        for section in &self.sections {
            let section_bytes = section.to_bytes();
            file.write_all(&section_bytes)?;
            outidx += section_bytes.len() as u32;
        }

        // Update header with new section header offset
        file.seek(SeekFrom::Start(0))?;
        let header_bytes = self.header.to_bytes(&self.fmt)?;
        file.write_all(&header_bytes)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_elf_file_read_write() -> Result<(), Error> {
        // Create test data
        let fmt = ElfFormat::new(true); // Big endian
        let mut data = vec![0; 0x1000];
        
        // ELF magic and identification
        data[0..4].copy_from_slice(b"\x7fELF");
        data[EI_CLASS] = 1; // 32-bit
        data[EI_DATA] = 2; // Big endian
        data[EI_VERSION] = 1; // Version
        data[EI_OSABI] = 0; // OS ABI
        data[EI_ABIVERSION] = 0; // ABI Version

        fmt.pack_u16(&mut data[16..18], 1)?; 
        fmt.pack_u16(&mut data[18..20], 8)?; // EM_MIPS

        // Set these values in the raw data
        fmt.pack_u32(&mut data[24..28], 1)?; // e_version
        fmt.pack_u16(&mut data[46..48], 40)?; // e_shentsize
        fmt.pack_u16(&mut data[48..50], 3)?; // e_shnum - now 3 sections
        
        // Create sections data
        let strtab_offset = 0x200;
        let symtab_offset = 0x300;
        
        // Create string table data
        let strtab_data = b"\0.strtab\0.symtab\0.test\0";
        println!("String table data: {:?}", strtab_data);
        data[strtab_offset..strtab_offset + strtab_data.len()].copy_from_slice(strtab_data);
        
        // Create symbol table data (just a null symbol)
        let symtab_data = vec![0; 16];
        data[symtab_offset..symtab_offset + symtab_data.len()].copy_from_slice(&symtab_data);
        
        // Create section headers at offset 0x100
        let sh_offset = 0x100;
        
        // Null section
        let mut null_section = ElfSection::default();
        null_section.sh_name = 0;
        data[sh_offset..sh_offset + 40].copy_from_slice(&null_section.to_bytes());
        
        // String table section
        let mut strtab = ElfSection::default();
        strtab.sh_type = SHT_STRTAB;
        strtab.sh_offset = strtab_offset as u32;
        strtab.sh_size = strtab_data.len() as u32;
        strtab.sh_name = 1; // Points to ".strtab" in the string table
        strtab.data = strtab_data.to_vec();
        println!("String table section data: {:?}", strtab.data);
        data[sh_offset + 40..sh_offset + 80].copy_from_slice(&strtab.to_bytes());
        
        // Symbol table section
        let mut symtab = ElfSection::default();
        symtab.sh_type = SHT_SYMTAB;
        symtab.sh_link = 1; // Link to string table
        symtab.sh_offset = symtab_offset as u32;
        symtab.sh_size = symtab_data.len() as u32;
        symtab.sh_entsize = 16;
        symtab.sh_name = 8; // Points to ".symtab" in the string table
        symtab.data = symtab_data.clone();
        data[sh_offset + 80..sh_offset + 120].copy_from_slice(&symtab.to_bytes());
        
        // Set section header offset in ELF header
        fmt.pack_u32(&mut data[32..36], sh_offset as u32)?; // e_shoff
        fmt.pack_u16(&mut data[50..52], 1)?; // e_shstrndx - points to strtab
        
        // Create ELF file from test data
        let mut elf = ElfFile::new(&data)?;
        println!("Created ELF file with {} sections", elf.sections.len());
        for (i, section) in elf.sections.iter().enumerate() {
            println!("Section {}: type={}, offset={}, size={}, data.len()={}", 
                    i, section.sh_type, section.sh_offset, section.sh_size, section.data.len());
            if section.sh_type == SHT_STRTAB {
                println!("String table data after init: {:?}", section.data);
            }
        }
        
        // Add a new section
        let new_section_idx = elf.add_section(
            ".test2",
            SHT_PROGBITS,
            0,
            0,
            0,
            4,
            0,
            vec![1, 2, 3, 4]
        )?;
        
        // Write to temporary file
        let temp_file = "test_elf.tmp";
        elf.write(temp_file)?;
        
        // Read back and verify
        let data = fs::read(temp_file)?;
        let elf2 = ElfFile::new(&data)?;
        
        assert_eq!(elf2.sections.len(), elf.sections.len());
        assert_eq!(elf2.sections[new_section_idx].data, vec![1, 2, 3, 4]);
        
        // Clean up
        fs::remove_file(temp_file)?;
        
        Ok(())
    }
}
