use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use regex::Regex;
use lazy_static::lazy_static;

use crate::utils::error::{Error, Result};
use crate::utils::state::GlobalState;
use crate::asm::block::GlobalAsmBlock;
use crate::asm::function::Function;
use crate::utils::options::Opts;

lazy_static! {
    static ref CUTSCENE_DATA_RE: Regex = Regex::new(r"CutsceneData (.|\n)*\[\] = {").unwrap();
    static ref FLOAT_RE: Regex = Regex::new(r"[-+]?[0-9]*\.?[0-9]+([eE][-+]?[0-9]+)?f").unwrap();
}

/// Convert a float string to its hexadecimal representation
fn repl_float_hex(cap: &regex::Captures) -> String {
    let float_str = cap[0].trim().trim_end_matches('f');
    let float_val = float_str.parse::<f32>().unwrap();
    let hex_val = f32::to_be_bytes(float_val);
    format!("{}", u32::from_be_bytes(hex_val))
}

/// Parse source file and process assembly blocks
///
/// # Arguments
/// * `f` - Input file handle
/// * `opts` - Processing options
/// * `out_dependencies` - List to store dependencies
/// * `print_source` - Optional output writer for processed source
///
/// # Returns
/// * Vector of processed assembly functions
pub fn parse_source<R: BufRead, W: Write>(
    f: &mut R,
    opts: &Opts,
    out_dependencies: &mut Vec<String>,
    print_source: Option<&mut W>,
) -> Result<Vec<Function>> {
    // Calculate instruction counts based on optimization level
    let (min_instr_count, skip_instr_count, prelude_if_late_rodata) = match (opts.opt.as_str(), opts.framepointer) {
        ("O1" | "O2", true) => (6, 5, 0),
        ("O1" | "O2", false) => (2, 1, 0),
        ("O0", true) => (8, 8, 0),
        ("O0", false) => (4, 4, 0),
        ("g", true) => (7, 7, 0),
        ("g", false) => (4, 4, 0),
        ("g3", true) => (4, 4, 0),
        ("g3", false) => (2, 2, 0),
        _ => return Err(Error::InvalidInput("must pass one of -g, -O0, -O1, -O2, -O2 -g3".into())),
    };

    // Adjust for KPIC
    let (min_instr_count, skip_instr_count, prelude_if_late_rodata) = if opts.kpic {
        if opts.opt == "g3" || opts.opt == "O2" {
            (min_instr_count, skip_instr_count, 3)
        } else {
            (min_instr_count + 3, skip_instr_count + 3, prelude_if_late_rodata)
        }
    } else {
        (min_instr_count, skip_instr_count, prelude_if_late_rodata)
    };

    let use_jtbl_for_rodata = opts.opt.as_str() == "O2" || opts.opt.as_str() == "g3" 
        && !opts.framepointer && !opts.kpic;

    let mut state = GlobalState::new(
        min_instr_count,
        skip_instr_count,
        use_jtbl_for_rodata,
        prelude_if_late_rodata,
        opts.mips1,
        opts.pascal,
    );

    let mut global_asm: Option<GlobalAsmBlock> = None;
    let mut asm_functions = Vec::new();
    let mut output_lines = vec![format!("#line 1 \"{}\"", opts.filename.display())];
    let mut is_cutscene_data = false;
    let mut is_early_include = false;

    let mut line_no = 1;
    let mut line = String::new();
    while f.read_line(&mut line)? > 0 {
        let raw_line = line.trim_end().to_string();
        let trimmed_line = raw_line.trim_start();

        // Ensure one output line per source line
        output_lines.push(String::new());
        let current_line_idx = output_lines.len() - 1;

        if let Some(ref mut asm_block) = global_asm {
            if trimmed_line.starts_with(')') {
                let (src, func) = asm_block.clone().finish(&mut state)?;
                let start_index = current_line_idx - src.len() + 1;
                for (i, line2) in src.into_iter().enumerate() {
                    output_lines[start_index + i] = line2;
                }
                asm_functions.push(func);
                global_asm = None;
            } else {
                asm_block.process_line(&raw_line, &opts.output_enc)?;
            }
        } else if trimmed_line == "GLOBAL_ASM(" || trimmed_line == "#pragma GLOBAL_ASM(" {
            global_asm = Some(GlobalAsmBlock::new(&format!("GLOBAL_ASM block at line {}", line_no)));
        } else if (trimmed_line.starts_with("GLOBAL_ASM(\"") || trimmed_line.starts_with("#pragma GLOBAL_ASM(\""))
            && trimmed_line.ends_with("\")") 
            || (trimmed_line.starts_with("INCLUDE_ASM(\"") || trimmed_line.starts_with("INCLUDE_RODATA(\""))
            && trimmed_line.contains("\",")
            && trimmed_line.ends_with(");") {
            
            let mut prologue = Vec::new();
            let fname = if trimmed_line.starts_with("INCLUDE_") {
                let parts: Vec<&str> = trimmed_line.split("\",").collect();
                let before = parts[0];
                let after = parts[1].trim()[..parts[1].trim().len()-2].to_string();
                let path = &before[before.find('(').unwrap() + 2..];
                if trimmed_line.starts_with("INCLUDE_RODATA") {
                    prologue.push(".section .rodata".to_string());
                }
                format!("{}/{}.s", path, after)
            } else {
                trimmed_line[trimmed_line.find('(').unwrap() + 2..trimmed_line.len()-2].to_string()
            };

            let mut ext_global_asm = GlobalAsmBlock::new(&fname);
            for line2 in prologue {
                ext_global_asm.process_line(&line2, &opts.output_enc)?;
            }

            match File::open(&fname) {
                Ok(file) => {
                    let reader = BufReader::new(file);
                    for line2 in reader.lines() {
                        let line2 = line2?;
                        ext_global_asm.process_line(&line2, &opts.output_enc)?;
                    }
                    let (src, func) = ext_global_asm.finish(&mut state)?;
                    output_lines[current_line_idx] = src.join("");
                    asm_functions.push(func);
                    out_dependencies.push(fname);
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    output_lines[current_line_idx] = format!("#include \"GLOBAL_ASM:{}\"", fname);
                }
                Err(e) => return Err(Error::Io(e)),
            }
        } else if trimmed_line == "#pragma asmproc recurse" {
            is_early_include = true;
        } else if is_early_include {
            is_early_include = false;
            if !trimmed_line.starts_with("#include ") {
                return Err(Error::InvalidInput("#pragma asmproc recurse must be followed by an #include".into()));
            }
            let include_path = trimmed_line[trimmed_line.find(' ').unwrap() + 2..trimmed_line.len()-1].to_string();
            let fpath = Path::new(&opts.filename).parent().unwrap_or_else(|| Path::new(""));
            let fname = fpath.join(include_path);
            out_dependencies.push(fname.to_string_lossy().into_owned());

            let mut include_file = File::open(&fname)
                .map_err(|e| Error::Io(e))?;
            let mut include_src = Vec::new();
            parse_source(
                &mut BufReader::new(&mut include_file),
                opts,
                out_dependencies,
                Some(&mut include_src),
            )?;
            writeln!(include_src, "#line {} \"{}\"", line_no + 1, opts.filename.display())?;
            output_lines[current_line_idx] = String::from_utf8(include_src)
                .map_err(|_| Error::InvalidInput("Invalid UTF-8 in included file".into()))?;
        } else {
            if opts.enable_cutscene_data_float_encoding {
                if CUTSCENE_DATA_RE.is_match(trimmed_line) {
                    is_cutscene_data = true;
                } else if trimmed_line.ends_with("};") {
                    is_cutscene_data = false;
                }
                if is_cutscene_data {
                    output_lines[current_line_idx] = FLOAT_RE.replace_all(&raw_line, repl_float_hex).into_owned();
                    line.clear();
                    line_no += 1;
                    continue;
                }
            }
            output_lines[current_line_idx] = raw_line;
        }

        line.clear();
        line_no += 1;
    }

    if let Some(print_source) = print_source {
        for line in &output_lines {
            write!(print_source, "{}\n", line)?;
        }
        print_source.flush()?;
    }

    Ok(asm_functions)
}
