#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub header: SectionHeader,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SectionHeader {
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
}
