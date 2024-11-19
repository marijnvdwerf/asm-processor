#[derive(Debug, Clone, Copy)]
pub struct ElfFormat {
    pub is_big_endian: bool,
}

impl ElfFormat {
    pub fn new(is_big_endian: bool) -> Self {
        Self { is_big_endian }
    }
}
