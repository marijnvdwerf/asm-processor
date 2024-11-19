use crate::elf::format::ElfFormat;
use crate::elf::header::ElfHeader;
use crate::elf::section::ElfSection;

#[derive(Debug)]
pub struct ElfFile {
    pub header: ElfHeader,
    pub sections: Vec<ElfSection>,
    pub fmt: ElfFormat,
}

// TODO: Implement ElfFile
