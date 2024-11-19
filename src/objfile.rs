use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use tempfile::NamedTempFile;

use crate::elf::{
    file::ElfFile,
    symbol::Symbol,
    constants::{
        SHN_UNDEF, SHT_RELA, SHN_ABS, STT_FUNC, STB_LOCAL, STT_OBJECT,
        STV_DEFAULT, STB_GLOBAL, SHT_REL,
    },
};
use crate::error::Error;

const SECTIONS: &[&str] = &[".data", ".text", ".rodata", ".bss"];

/// Represents a function's assembly data
#[derive(Debug)]
pub struct AsmFunction {
    pub data: HashMap<String, (Option<String>, usize)>,
    pub asm_contents: Vec<String>,
    pub text_glabels: Vec<String>,
    pub late_rodata_asm_contents: Vec<String>,
    pub late_rodata_dummy_bytes: Vec<Vec<u8>>,
    pub jtbl_rodata_size: usize,
    pub fn_desc: String,
}

/// Check if a symbol name is a temporary name
fn is_temp_name(name: &str) -> bool {
    name.starts_with("_asmpp_")
}

/// Fix up an object file by processing its assembly and merging it with C code
///
/// # Arguments
/// * `objfile_path` - Path to the object file to process
/// * `functions` - Assembly functions to process
/// * `asm_prelude` - Assembly prelude content
/// * `assembler` - Assembler command to use
/// * `output_enc` - Output encoding
/// * `drop_mdebug_gptab` - Whether to drop mdebug and gptab sections
/// * `convert_statics` - How to handle static symbols
///
/// # Returns
/// * `Result<(), Error>` - Success or error
pub fn fixup_objfile(
    objfile_path: &Path,
    functions: &[AsmFunction],
    asm_prelude: &[u8],
    assembler: &str,
    output_enc: &str,
    drop_mdebug_gptab: bool,
    convert_statics: &str,
) -> Result<(), Error> {
    // Read the object file
    let mut objfile = ElfFile::from_file(objfile_path)?;
    let fmt = objfile.fmt();

    // Track previous locations and sections to copy
    let mut prev_locs: HashMap<&str, usize> = SECTIONS.iter().map(|&s| (s, 0)).collect();
    let mut to_copy: HashMap<&str, Vec<(usize, usize, String, String)>> = 
        SECTIONS.iter().map(|&s| (s, Vec::new())).collect();

    // Assembly generation state
    let mut asm = Vec::new();
    let mut all_text_glabels = HashSet::new();
    let mut all_late_rodata_dummy_bytes = Vec::new();
    let mut all_jtbl_rodata_size = Vec::new();
    let mut late_rodata_asm = Vec::new();
    let mut func_sizes = HashMap::new();

    // Process each function
    for function in functions {
        let mut ifdefed = false;
        
        // Process each section in the function
        for (sectype, (temp_name, size)) in &function.data {
            if let Some(temp_name) = temp_name {
                if size == &0 {
                    continue;
                }
                
                // Find symbol location
                let loc = match objfile.symtab.find_symbol(temp_name) {
                    Some((_, loc)) => loc,
                    None => {
                        ifdefed = true;
                        break;
                    }
                };

                let prev_loc = *prev_locs.get(sectype.as_str()).unwrap();
                if loc < prev_loc {
                    return Err(Error::ProcessingError(format!(
                        "Wrongly computed size for section {} (diff {})",
                        sectype, prev_loc - loc
                    )));
                }

                // Add padding if needed
                if loc != prev_loc {
                    asm.push(format!(".section {}", sectype));
                    if sectype == ".text" {
                        for _ in 0..((loc - prev_loc) / 4) {
                            asm.push("nop".to_string());
                        }
                    } else {
                        asm.push(format!(".space {}", loc - prev_loc));
                    }
                }

                to_copy.get_mut(sectype.as_str()).unwrap().push((
                    loc,
                    *size,
                    temp_name.clone(),
                    function.fn_desc.clone(),
                ));

                if !function.text_glabels.is_empty() && sectype == ".text" {
                    func_sizes.insert(function.text_glabels[0].clone(), *size);
                }

                prev_locs.insert(sectype.as_str(), loc + size);
            }
        }

        if !ifdefed {
            all_text_glabels.extend(function.text_glabels.iter().cloned());
            all_late_rodata_dummy_bytes.push(function.late_rodata_dummy_bytes.clone());
            all_jtbl_rodata_size.push(function.jtbl_rodata_size);
            late_rodata_asm.push(function.late_rodata_asm_contents.clone());

            // Add section labels
            for (sectype, (temp_name, _)) in &function.data {
                if let Some(temp_name) = temp_name {
                    asm.push(format!(".section {}", sectype));
                    asm.push(format!("glabel {}_asm_start", temp_name));
                }
            }

            // Add function assembly
            asm.push(".text".to_string());
            asm.extend(function.asm_contents.iter().cloned());

            // Add end labels
            for (sectype, (temp_name, _)) in &function.data {
                if let Some(temp_name) = temp_name {
                    asm.push(format!(".section {}", sectype));
                    asm.push(format!("glabel {}_asm_end", temp_name));
                }
            }
        }
    }

    // Handle late rodata if present
    if late_rodata_asm.iter().any(|x| !x.is_empty()) {
        asm.push(".section .late_rodata".to_string());
        asm.push(".word 0, 0".to_string());
        asm.push("glabel _asmpp_late_rodata_start".to_string());
        for contents in late_rodata_asm {
            asm.extend(contents);
        }
        asm.push("glabel _asmpp_late_rodata_end".to_string());
    }

    // Create temporary files
    let mut o_file = NamedTempFile::new()?;
    let o_path = o_file.path().to_owned();
    let mut s_file = NamedTempFile::new()?;

    // Write assembly file
    s_file.write_all(asm_prelude)?;
    s_file.write_all(b"\n")?;
    for line in asm {
        s_file.write_all(line.as_bytes())?;
        s_file.write_all(b"\n")?;
    }
    s_file.flush()?;

    // Run assembler
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{} {} -o {}", 
            assembler,
            s_file.path().display(),
            o_path.display()
        ))
        .status()?;

    if !status.success() {
        return Err(Error::ProcessingError("Failed to assemble".to_string()));
    }

    // Read assembled object file
    let asm_objfile = ElfFile::from_file(&o_path)?;

    // Process sections, symbols, and relocations
    process_sections(&mut objfile, &asm_objfile, &to_copy, &all_text_glabels)?;
    process_symbols(&mut objfile, &asm_objfile, convert_statics, &all_text_glabels)?;
    process_relocations(&mut objfile, &asm_objfile)?;

    // Write final object file
    objfile.write(objfile_path)?;

    Ok(())
}

// Helper functions for processing different parts of the object file
fn process_sections(
    objfile: &mut ElfFile,
    asm_objfile: &ElfFile,
    to_copy: &HashMap<&str, Vec<(usize, usize, String, String)>>,
    all_text_glabels: &HashSet<String>,
) -> Result<(), Error> {
    let mut modified_text_positions = HashSet::new();
    let mut jtbl_rodata_positions = HashSet::new();
    let mut last_rodata_pos = 0;

    // Process each section type
    for &sectype in SECTIONS {
        if to_copy[sectype].is_empty() {
            continue;
        }

        let source = asm_objfile.find_section(sectype)
            .ok_or_else(|| Error::ProcessingError(format!("didn't find source section: {}", sectype)))?;

        // Verify positions and sizes
        for (pos, count, temp_name, fn_desc) in &to_copy[sectype] {
            let loc1 = asm_objfile.symtab.find_symbol_in_section(&format!("{}_asm_start", temp_name), &source)
                .ok_or_else(|| Error::ProcessingError("symbol not found".to_string()))?;
            let loc2 = asm_objfile.symtab.find_symbol_in_section(&format!("{}_asm_end", temp_name), &source)
                .ok_or_else(|| Error::ProcessingError("symbol not found".to_string()))?;

            if loc1 != *pos {
                return Err(Error::ProcessingError(format!(
                    "assembly and C files don't line up for section {}, {}", 
                    sectype, fn_desc
                )));
            }

            if loc2 - loc1 != *count {
                return Err(Error::ProcessingError(format!(
                    "incorrectly computed size for section {}, {}. If using .double, make sure to provide explicit alignment padding.",
                    sectype, fn_desc
                )));
            }
        }

        // Skip .bss section as it contains no data
        if sectype == ".bss" {
            continue;
        }

        // Process section data
        let target = objfile.find_section(sectype)
            .ok_or_else(|| Error::ProcessingError(format!("missing target section of type {}", sectype)))?;

        let mut data = target.data.to_vec();
        for (pos, count, _, _) in &to_copy[sectype] {
            let start = *pos;
            let end = start + count;
            data[start..end].copy_from_slice(&source.data[start..end]);

            if sectype == ".text" {
                if count % 4 != 0 || pos % 4 != 0 {
                    return Err(Error::ProcessingError("text section misaligned".to_string()));
                }
                for i in 0..(count / 4) {
                    modified_text_positions.insert(pos + 4 * i);
                }
            } else if sectype == ".rodata" {
                last_rodata_pos = pos + count;
            }
        }
        target.data = data;
    }

    Ok(())
}

fn process_symbols(
    objfile: &mut ElfFile,
    asm_objfile: &ElfFile,
    convert_statics: &str,
    all_text_glabels: &HashSet<String>,
) -> Result<(), Error> {
    // Merge strtab data
    let strtab_adj = objfile.symtab.strtab.data.len();
    objfile.symtab.strtab.data.extend(&asm_objfile.symtab.strtab.data);

    // Find relocated symbols
    let mut relocated_symbols = HashSet::new();
    for &sectype in SECTIONS.iter().chain(&[".late_rodata"]) {
        for obj in &[asm_objfile, objfile] {
            if let Some(sec) = obj.find_section(sectype) {
                for reltab in &sec.relocated_by {
                    for rel in &reltab.relocations {
                        relocated_symbols.insert(obj.symtab.symbol_entries[rel.sym_index].clone());
                    }
                }
            }
        }
    }

    // Process symbols
    let empty_symbol = objfile.symtab.symbol_entries[0].clone();
    let mut new_syms: Vec<Symbol> = objfile.symtab.symbol_entries[1..]
        .iter()
        .filter(|s| !is_temp_name(&s.name))
        .cloned()
        .collect();

    for (i, s) in asm_objfile.symtab.symbol_entries.iter().enumerate() {
        let is_local = i < asm_objfile.symtab.sh_info;
        
        // Skip local unrelocated and temporary symbols
        if (is_local && !relocated_symbols.contains(s)) || is_temp_name(&s.name) {
            continue;
        }

        let mut sym = s.clone();
        
        // Process non-special sections
        if sym.st_shndx != SHN_UNDEF && sym.st_shndx != SHN_ABS {
            let section_name = asm_objfile.sections[sym.st_shndx].name.clone();
            let target_section_name = if section_name == ".late_rodata" {
                ".rodata".to_string()
            } else if !SECTIONS.contains(&section_name.as_str()) {
                return Err(Error::ProcessingError(format!(
                    "generated assembly .o must only have symbols for .text, .data, .rodata, .late_rodata, ABS and UNDEF, but found {}",
                    section_name
                )));
            } else {
                section_name.clone()
            };

            let objfile_section = objfile.find_section(&target_section_name)
                .ok_or_else(|| Error::ProcessingError(format!(
                    "generated assembly .o has section that real objfile lacks: {}", 
                    target_section_name
                )))?;

            sym.st_shndx = objfile_section.index;

            // Handle text glabels
            if all_text_glabels.contains(&sym.name) {
                sym.type_ = STT_FUNC;
            }
        }

        sym.st_name += strtab_adj;
        new_syms.push(sym);
    }

    // Sort and deduplicate symbols
    new_syms.sort_by_key(|s| (s.st_shndx != SHN_UNDEF, s.name.clone()));
    let mut unique_syms = Vec::new();
    let mut name_to_sym = HashMap::new();

    for s in new_syms {
        if !s.name.is_empty() {
            if let Some(existing) = name_to_sym.get(&s.name) {
                if s.st_shndx != SHN_UNDEF && 
                   !(existing.st_shndx == s.st_shndx && existing.st_value == s.st_value) {
                    return Err(Error::ProcessingError(format!(
                        "symbol \"{}\" defined twice", s.name
                    )));
                }
                continue;
            }
            name_to_sym.insert(s.name.clone(), s.clone());
        }
        unique_syms.push(s);
    }

    // Update symbol table
    unique_syms.insert(0, empty_symbol);
    unique_syms.sort_by_key(|s| (s.bind != STB_LOCAL, s.name == "_gp_disp"));
    
    let num_local_syms = unique_syms.iter().filter(|s| s.bind == STB_LOCAL).count();
    objfile.symtab.symbol_entries = unique_syms;
    objfile.symtab.sh_info = num_local_syms;

    Ok(())
}

fn process_relocations(
    objfile: &mut ElfFile,
    asm_objfile: &ElfFile,
) -> Result<(), Error> {
    // Process relocations for each section
    for &sectype in SECTIONS {
        let target = match objfile.find_section(sectype) {
            Some(sec) => sec,
            None => continue,
        };

        // Update existing relocations
        for reltab in &mut target.relocated_by {
            let mut new_rels = Vec::new();
            
            for rel in &reltab.relocations {
                // Skip relocations for modified text positions
                if (sectype == ".text" && modified_text_positions.contains(&rel.r_offset)) ||
                   (sectype == ".rodata" && jtbl_rodata_positions.contains(&rel.r_offset)) {
                    continue;
                }
                
                let mut new_rel = rel.clone();
                new_rel.sym_index = objfile.symtab.symbol_entries[rel.sym_index].new_index;
                new_rels.push(new_rel);
            }
            
            reltab.relocations = new_rels;
            reltab.data = new_rels.iter().map(|r| r.to_bin()).collect();
        }

        // Move over new relocations from assembly
        let source = match asm_objfile.find_section(sectype) {
            Some(sec) => sec,
            None => continue,
        };

        let target_reltab = objfile.find_section(&format!(".rel{}", sectype));
        let target_reltaba = objfile.find_section(&format!(".rela{}", sectype));

        for reltab in &source.relocated_by {
            let mut new_rels = Vec::new();
            for rel in &reltab.relocations {
                let mut new_rel = rel.clone();
                new_rel.sym_index = asm_objfile.symtab.symbol_entries[rel.sym_index].new_index;
                new_rels.push(new_rel);
            }

            let new_data: Vec<u8> = new_rels.iter().map(|r| r.to_bin()).collect();

            match reltab.sh_type {
                SHT_REL => {
                    if let Some(target_rel) = target_reltab {
                        target_rel.data.extend(&new_data);
                    } else {
                        objfile.add_section(
                            &format!(".rel{}", sectype),
                            SHT_REL,
                            0,
                            objfile.symtab.index,
                            target.index,
                            4,
                            8,
                            new_data,
                        )?;
                    }
                }
                SHT_RELA => {
                    if let Some(target_rela) = target_reltaba {
                        target_rela.data.extend(&new_data);
                    } else {
                        objfile.add_section(
                            &format!(".rela{}", sectype),
                            SHT_RELA,
                            0,
                            objfile.symtab.index,
                            target.index,
                            4,
                            12,
                            new_data,
                        )?;
                    }
                }
                _ => return Err(Error::ProcessingError("unknown relocation type".to_string())),
            }
        }
    }

    Ok(())
}
