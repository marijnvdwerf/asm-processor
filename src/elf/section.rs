use crate::elf::format::ElfFormat;

pub trait Section {
    fn lookup_str(&self, offset: u32) -> String;
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
    pub name: String,
    pub data: Vec<u8>,
    pub fmt: ElfFormat,
}

// TODO: Implement ElfSection
