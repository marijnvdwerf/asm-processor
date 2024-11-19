use std::collections::HashMap;
use crate::utils::Result;

#[derive(Debug)]
pub struct GlobalAsmBlock {
    pub fn_desc: String,
    pub cur_section: String,
    pub asm_conts: Vec<String>,
    pub late_rodata_asm_conts: Vec<String>,
    pub late_rodata_alignment: usize,
    pub late_rodata_alignment_from_content: bool,
    pub text_glabels: Vec<String>,
    pub fn_section_sizes: HashMap<String, usize>,
}

impl GlobalAsmBlock {
    pub fn new(fn_desc: &str) -> Self {
        Self {
            fn_desc: fn_desc.to_string(),
            cur_section: ".text".to_string(),
            asm_conts: Vec::new(),
            late_rodata_asm_conts: Vec::new(),
            late_rodata_alignment: 0,
            late_rodata_alignment_from_content: false,
            text_glabels: Vec::new(),
            fn_section_sizes: HashMap::from([
                (".text".to_string(), 0),
                (".data".to_string(), 0),
                (".bss".to_string(), 0),
                (".rodata".to_string(), 0),
                (".late_rodata".to_string(), 0),
            ]),
        }
    }
}
