use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Function {
    pub text_glabels: Vec<String>,
    pub asm_conts: Vec<String>,
    pub late_rodata_dummy_bytes: Vec<String>,
    pub jtbl_rodata_size: usize,
    pub late_rodata_asm_conts: Vec<String>,
    pub fn_desc: String,
    pub data: HashMap<String, (String, usize)>,
}
