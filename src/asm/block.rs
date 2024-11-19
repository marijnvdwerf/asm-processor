use std::collections::HashMap;
use crate::utils::Result;
use crate::utils::errors::Failure;
use crate::utils::constants::MAX_FN_SIZE;
use crate::utils::state::GlobalState;
use crate::asm::function::Function;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref RE_COMMENT_OR_STRING: Regex = Regex::new(
        r#"#.*|/\*.*?\*/|"(?:\\.|[^\\"])*""#
    ).unwrap();
}

fn re_comment_replacer(cap: &regex::Captures) -> String {
    let s = cap.get(0).unwrap().as_str();
    if s.starts_with('#') || s.starts_with('/') {
        " ".to_string()
    } else {
        s.to_string()
    }
}

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
    fn_ins_inds: Vec<(usize, usize)>,
    glued_line: String,
    num_lines: usize,
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
            fn_ins_inds: Vec::new(),
            glued_line: String::new(),
            num_lines: 0,
        }
    }

    fn fail(&self, message: &str, line: Option<&str>) -> Failure {
        let context = if let Some(line_str) = line {
            format!("{}, at line \"{}\"", self.fn_desc, line_str)
        } else {
            self.fn_desc.clone()
        };
        Failure::new(&format!("{}\nwithin {}", message, context))
    }

    fn count_quoted_size(&self, line: &str, z: bool, real_line: &str, output_enc: &str) -> Result<usize> {
        // Convert line to bytes using output_enc, then back to latin1 for processing
        let line = String::from_utf8(line.as_bytes().to_vec())
            .map_err(|_| self.fail("Invalid UTF-8 in line", Some(real_line)))?;
        
        let mut in_quote = false;
        let mut has_comma = true;
        let mut num_parts = 0;
        let mut ret = 0;
        let mut i = 0;
        let chars: Vec<char> = line.chars().collect();
        let digits = "0123456789";

        while i < chars.len() {
            let c = chars[i];
            i += 1;
            if !in_quote {
                if c == '"' {
                    in_quote = true;
                    if z && !has_comma {
                        return Err(self.fail(".asciiz with glued strings is not supported due to GNU as version diffs", Some(real_line)));
                    }
                    num_parts += 1;
                } else if c == ',' {
                    has_comma = true;
                }
            } else {
                if c == '"' {
                    in_quote = false;
                    has_comma = false;
                    continue;
                }
                ret += 1;
                if c != '\\' {
                    continue;
                }
                if i == chars.len() {
                    return Err(self.fail("backslash at end of line not supported", Some(real_line)));
                }
                let c = chars[i];
                i += 1;
                if c == 'x' {
                    while i < chars.len() && (digits.contains(chars[i]) || "abcdefABCDEF".contains(chars[i])) {
                        i += 1;
                    }
                } else if digits.contains(c) {
                    let mut it = 0;
                    while i < chars.len() && digits.contains(chars[i]) && it < 2 {
                        i += 1;
                        it += 1;
                    }
                }
            }
        }

        if in_quote {
            return Err(self.fail("unterminated string literal", Some(real_line)));
        }
        if num_parts == 0 {
            return Err(self.fail(".ascii with no string", Some(real_line)));
        }
        Ok(if z { ret + num_parts } else { ret })
    }

    fn align2(&mut self) {
        while self.fn_section_sizes[&self.cur_section] % 2 != 0 {
            *self.fn_section_sizes.get_mut(&self.cur_section).unwrap() += 1;
        }
    }

    fn align4(&mut self) {
        while self.fn_section_sizes[&self.cur_section] % 4 != 0 {
            *self.fn_section_sizes.get_mut(&self.cur_section).unwrap() += 1;
        }
    }

    fn add_sized(&mut self, size: isize, line: &str) -> Result<()> {
        if self.cur_section == ".text" || self.cur_section == ".late_rodata" {
            if size % 4 != 0 {
                return Err(self.fail("size must be a multiple of 4", Some(line)));
            }
        }
        if size < 0 {
            return Err(self.fail("size cannot be negative", Some(line)));
        }
        *self.fn_section_sizes.get_mut(&self.cur_section).unwrap() += size as usize;
        
        if self.cur_section == ".text" {
            if self.text_glabels.is_empty() {
                return Err(self.fail(".text block without an initial glabel", Some(line)));
            }
            self.fn_ins_inds.push((self.num_lines - 1, size as usize / 4));
        }
        Ok(())
    }

    pub fn process_line(&mut self, line: &str, output_enc: &str) -> Result<()> {
        self.num_lines += 1;
        
        // Handle line continuation
        if line.ends_with('\\') {
            self.glued_line.push_str(&line[..line.len()-1]);
            return Ok(());
        }
        
        let mut line = self.glued_line.clone() + line;
        self.glued_line.clear();

        let real_line = line.clone();
        // Replace comments and strings
        line = RE_COMMENT_OR_STRING.replace_all(&line, re_comment_replacer).to_string();
        line = line.trim().to_string();
        
        // Remove label definitions
        line = regex::Regex::new(r"^[a-zA-Z0-9_]+:\s*")
            .unwrap()
            .replace(&line, "")
            .to_string();

        let mut changed_section = false;
        let mut emitting_double = false;

        if line.is_empty() {
            // Empty line, nothing to do
        } else if (line.starts_with("glabel ") || line.starts_with("jlabel ")) && self.cur_section == ".text" {
            self.text_glabels.push(line.split_whitespace().nth(1).unwrap().to_string());
        } else if line.starts_with("glabel ") || line.starts_with("dlabel ") || 
                 line.starts_with("jlabel ") || line.starts_with("endlabel ") || 
                 (!line.contains(' ') && line.ends_with(':')) {
            // Label, nothing to do
        } else if line.starts_with(".section") || [".text", ".data", ".rdata", ".rodata", ".bss", ".late_rodata"].contains(&line.as_str()) {
            // Section change
            self.cur_section = if line == ".rdata" { 
                ".rodata".to_string() 
            } else { 
                line.split(',').next().unwrap().split_whitespace().last().unwrap().to_string() 
            };
            
            if !vec![".data", ".text", ".rodata", ".late_rodata", ".bss"].contains(&self.cur_section.as_str()) {
                return Err(self.fail("unrecognized .section directive", Some(&real_line)));
            }
            changed_section = true;
        } else if line.starts_with(".late_rodata_alignment") {
            if self.cur_section != ".late_rodata" {
                return Err(self.fail(".late_rodata_alignment must occur within .late_rodata section", Some(&real_line)));
            }
            let value: usize = line.split_whitespace().nth(1).unwrap().parse().unwrap();
            if value != 4 && value != 8 {
                return Err(self.fail(".late_rodata_alignment argument must be 4 or 8", Some(&real_line)));
            }
            if self.late_rodata_alignment != 0 && self.late_rodata_alignment != value {
                return Err(self.fail(".late_rodata_alignment alignment assumption conflicts with earlier .double directive. Make sure to provide explicit alignment padding.", None));
            }
            self.late_rodata_alignment = value;
            changed_section = true;
        } else if line.starts_with(".incbin") {
            let size = line.split(',').last().unwrap().trim().parse::<isize>().unwrap();
            self.add_sized(size, &real_line)?;
        } else if line.starts_with(".word") || line.starts_with(".gpword") || line.starts_with(".float") {
            self.align4();
            let count = line.split(',').count();
            self.add_sized((4 * count) as isize, &real_line)?;
        } else if line.starts_with(".double") {
            self.align4();
            if self.cur_section == ".late_rodata" {
                let align8 = self.fn_section_sizes[&self.cur_section] % 8;
                if self.late_rodata_alignment == 0 {
                    self.late_rodata_alignment = 8 - align8;
                    self.late_rodata_alignment_from_content = true;
                } else if self.late_rodata_alignment != 8 - align8 {
                    if self.late_rodata_alignment_from_content {
                        return Err(self.fail("found two .double directives with different start addresses mod 8. Make sure to provide explicit alignment padding.", Some(&real_line)));
                    } else {
                        return Err(self.fail(".double at address that is not 0 mod 8 (based on .late_rodata_alignment assumption). Make sure to provide explicit alignment padding.", Some(&real_line)));
                    }
                }
            }
            let count = line.split(',').count();
            self.add_sized((8 * count) as isize, &real_line)?;
            emitting_double = true;
        } else if line.starts_with(".space") {
            let size = line.split_whitespace().nth(1).unwrap().parse::<isize>().unwrap();
            self.add_sized(size, &real_line)?;
        } else if line.starts_with(".balign") {
            let align = line.split_whitespace().nth(1).unwrap().parse::<usize>().unwrap();
            if align != 4 {
                return Err(self.fail("only .balign 4 is supported", Some(&real_line)));
            }
            self.align4();
        } else if line.starts_with(".align") {
            let align = line.split_whitespace().nth(1).unwrap().parse::<usize>().unwrap();
            if align != 2 {
                return Err(self.fail("only .align 2 is supported", Some(&real_line)));
            }
            self.align4();
        } else if line.starts_with(".asci") {
            let z = line.starts_with(".asciz") || line.starts_with(".asciiz");
            let size = self.count_quoted_size(&line, z, &real_line, output_enc)?;
            self.add_sized(size as isize, &real_line)?;
        } else {
            // Instruction or macro
            if self.cur_section != ".text" {
                return Err(self.fail("instruction or macro call in non-.text section? not supported", Some(&real_line)));
            }
            self.add_sized(4, &real_line)?;
        }

        if self.cur_section == ".late_rodata" {
            if !changed_section {
                if emitting_double {
                    self.late_rodata_asm_conts.push(".align 0".to_string());
                }
                self.late_rodata_asm_conts.push(real_line);
                if emitting_double {
                    self.late_rodata_asm_conts.push(".align 2".to_string());
                }
            }
        } else {
            self.asm_conts.push(real_line);
        }

        Ok(())
    }

    pub fn finish(&mut self, state: &mut GlobalState) -> Result<(Vec<String>, Function)> {
        let mut src = vec!["".to_string(); self.num_lines + 1];
        let mut late_rodata_dummy_bytes = Vec::new();
        let mut jtbl_rodata_size = 0;
        let mut late_rodata_fn_output = Vec::new();

        let num_instr = self.fn_section_sizes[".text"] / 4;

        if self.fn_section_sizes[".late_rodata"] > 0 {
            let size = self.fn_section_sizes[".late_rodata"] / 4;
            let mut skip_next = false;
            let mut needs_double = self.late_rodata_alignment != 0;
            let mut extra_mips1_nop = false;
            
            let (jtbl_size, jtbl_min_rodata_size) = if state.pascal {
                (if state.mips1 { 9 } else { 8 }, 2)
            } else {
                (if state.mips1 { 11 } else { 9 }, 5)
            };

            let mut i = 0;
            while i < size {
                if skip_next {
                    skip_next = false;
                    i += 1;
                    continue;
                }

                if !needs_double && state.use_jtbl_for_rodata && i >= 1 && 
                   size - i >= jtbl_min_rodata_size && 
                   num_instr - late_rodata_fn_output.len() >= jtbl_size + 1 {
                    let line = if state.pascal {
                        let cases = (0..size-i).map(|case| format!("{}: ;", case)).collect::<Vec<_>>().join(" ");
                        format!("case 0 of {} otherwise end;", cases)
                    } else {
                        let cases = (0..size-i).map(|case| format!("case {}:", case)).collect::<Vec<_>>().join(" ");
                        format!("switch (*(volatile int*)0) {{ {} ; }}", cases)
                    };
                    late_rodata_fn_output.push(line);
                    late_rodata_fn_output.extend(vec!["".to_string(); jtbl_size - 1]);
                    jtbl_rodata_size = (size - i) * 4;
                    extra_mips1_nop = i != 2;
                    break;
                }

                let dummy_bytes = state.next_late_rodata_hex();
                late_rodata_dummy_bytes.push(dummy_bytes.clone());

                if self.late_rodata_alignment == 4 * ((i + 1) % 2 + 1) && i + 1 < size {
                    let dummy_bytes2 = state.next_late_rodata_hex();
                    late_rodata_dummy_bytes.push(dummy_bytes2.clone());
                    let fval = state.bytes_to_double(&(dummy_bytes + &dummy_bytes2));
                    let line = if state.pascal {
                        state.pascal_assignment("d", fval)
                    } else {
                        format!("*(volatile double*)0 = {};", fval)
                    };
                    late_rodata_fn_output.push(line);
                    skip_next = true;
                    needs_double = false;
                    if state.mips1 {
                        late_rodata_fn_output.push("".to_string());
                        late_rodata_fn_output.push("".to_string());
                    }
                    extra_mips1_nop = false;
                } else {
                    let fval = state.bytes_to_float(&dummy_bytes);
                    let line = if state.pascal {
                        state.pascal_assignment("f", fval)
                    } else {
                        format!("*(volatile float*)0 = {}f;", fval)
                    };
                    late_rodata_fn_output.push(line);
                    extra_mips1_nop = true;
                }
                late_rodata_fn_output.push("".to_string());
                late_rodata_fn_output.push("".to_string());
                i += 1;
            }
            if state.mips1 && extra_mips1_nop {
                late_rodata_fn_output.push("".to_string());
            }
        }

        let mut text_name = None;
        if self.fn_section_sizes[".text"] > 0 || !late_rodata_fn_output.is_empty() {
            text_name = Some(state.make_name("func"));
            src[0] = state.func_prologue(&text_name.as_ref().unwrap());
            src[self.num_lines] = state.func_epilogue();
            let instr_count = self.fn_section_sizes[".text"] / 4;
            if instr_count < state.min_instr_count {
                return Err(self.fail("too short .text block", None));
            }

            let mut tot_emitted = 0;
            let mut tot_skipped = 0;
            let mut fn_emitted = 0;
            let mut fn_skipped = 0;
            let mut skipping = true;
            let mut rodata_stack = late_rodata_fn_output.clone();
            rodata_stack.reverse();

            for (lineno, count) in &self.fn_ins_inds {
                for _ in 0..*count {
                    if fn_emitted > MAX_FN_SIZE && instr_count - tot_emitted > state.min_instr_count &&
                       (rodata_stack.is_empty() || !rodata_stack.last().unwrap().is_empty()) {
                        fn_emitted = 0;
                        fn_skipped = 0;
                        skipping = true;
                        src[*lineno] += &format!(" {} {} ",
                            state.func_epilogue(),
                            state.func_prologue(&state.make_name("large_func")));
                    }
                    if skipping && fn_skipped < state.skip_instr_count +
                        (if !rodata_stack.is_empty() { state.prelude_if_late_rodata } else { 0 }) {
                        fn_skipped += 1;
                        tot_skipped += 1;
                    } else {
                        skipping = false;
                        if !rodata_stack.is_empty() {
                            src[*lineno] += &rodata_stack.pop().unwrap();
                        } else if state.pascal {
                            src[*lineno] += &state.pascal_assignment("i", "0");
                        } else {
                            src[*lineno] += "*(volatile int*)0 = 0;";
                        }
                    }
                    tot_emitted += 1;
                    fn_emitted += 1;
                }
            }

            if !rodata_stack.is_empty() {
                let size = late_rodata_fn_output.len() / 3;
                let available = instr_count - tot_skipped;
                return Err(self.fail(&format!(
                    "late rodata to text ratio is too high: {} / {} must be <= 1/3\n\
                     add .late_rodata_alignment (4|8) to the .late_rodata block to double the allowed ratio.",
                    size, available
                ), None));
            }
        }

        let mut rodata_name = None;
        if self.fn_section_sizes[".rodata"] > 0 {
            if state.pascal {
                return Err(self.fail(".rodata isn't supported with Pascal for now", None));
            }
            rodata_name = Some(state.make_name("rodata"));
            src[self.num_lines] += &format!(" const char {}[{}] = {{1}};",
                rodata_name.as_ref().unwrap(),
                self.fn_section_sizes[".rodata"]);
        }

        let mut data_name = None;
        if self.fn_section_sizes[".data"] > 0 {
            data_name = Some(state.make_name("data"));
            let line = if state.pascal {
                format!(" var {}: packed array[1..{}] of char := [otherwise: 0];",
                    data_name.as_ref().unwrap(),
                    self.fn_section_sizes[".data"])
            } else {
                format!(" char {}[{}] = {{1}};",
                    data_name.as_ref().unwrap(),
                    self.fn_section_sizes[".data"])
            };
            src[self.num_lines] += &line;
        }

        let mut bss_name = None;
        if self.fn_section_sizes[".bss"] > 0 {
            if state.pascal {
                return Err(self.fail(".bss isn't supported with Pascal", None));
            }
            bss_name = Some(state.make_name("bss"));
            src[self.num_lines] += &format!(" char {}[{}];",
                bss_name.as_ref().unwrap(),
                self.fn_section_sizes[".bss"]);
        }

        let data = [
            (".text", text_name.map_or((String::new(), 0), |n| (n, self.fn_section_sizes[".text"]))),
            (".data", data_name.map_or((String::new(), 0), |n| (n, self.fn_section_sizes[".data"]))),
            (".rodata", rodata_name.map_or((String::new(), 0), |n| (n, self.fn_section_sizes[".rodata"]))),
            (".bss", bss_name.map_or((String::new(), 0), |n| (n, self.fn_section_sizes[".bss"]))),
        ].iter().map(|(k, v)| (k.to_string(), v.clone())).collect();

        Ok((src, Function {
            text_glabels: self.text_glabels.clone(),
            asm_conts: self.asm_conts.clone(),
            late_rodata_dummy_bytes,
            jtbl_rodata_size,
            late_rodata_asm_conts: self.late_rodata_asm_conts.clone(),
            fn_desc: self.fn_desc.clone(),
            data,
        }))
    }
}
