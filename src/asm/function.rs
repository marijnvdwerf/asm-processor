#[derive(Debug, Clone, Default)]
pub struct Function {
    pub text_size: usize,
    pub data_size: usize,
    pub rodata_size: usize,
    pub bss_size: usize,
    pub late_rodata_size: usize,
}
