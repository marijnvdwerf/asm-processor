use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::Path;
use tempfile::NamedTempFile;

use crate::elf::{
    Symbol,
    constants::{
        MIPS_DEBUG_ST_STATIC, MIPS_DEBUG_ST_STATIC_PROC, MIPS_DEBUG_ST_FILE,
        MIPS_DEBUG_ST_STRUCT, MIPS_DEBUG_ST_UNION, MIPS_DEBUG_ST_ENUM,
        MIPS_DEBUG_ST_BLOCK, MIPS_DEBUG_ST_PROC, MIPS_DEBUG_ST_END,
        STT_FUNC, STT_OBJECT, STB_LOCAL, STB_GLOBAL, STV_DEFAULT,
        SHN_UNDEF, SHT_REL, SHT_RELA
    }
};

use crate::elf::file::ElfFile;
use crate::utils::Error as CrateError;
use crate::asm::Function;

const SECTIONS: &[&str] = &[".data", ".text", ".rodata", ".bss"];

/// Error type for object file processing operations
#[derive(Debug, thiserror::Error)]
pub enum ObjFileError {
    /// IO errors during file operations
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    /// Errors during ELF file processing
    #[error("ELF processing error: {0}")]
    ElfError(String),
    
    /// Errors related to section processing
    #[error("Section error: {0}")]
    SectionError(String),
    
    /// Errors related to symbol processing
    #[error("Symbol error: {0}")]
    SymbolError(String),
    
    /// Errors related to relocation processing
    #[error("Relocation error: {0}")]
    RelocationError(String),
    
    /// Conversion errors
    #[error("Conversion error: {0}")]
    ConversionError(String),
}

impl From<CrateError> for ObjFileError {
    fn from(err: CrateError) -> Self {
        ObjFileError::ElfError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ObjFileError>;

/// Check if a symbol name is a temporary name
pub fn is_temp_name(name: &str) -> bool {
    name.starts_with("_asmpp_")
}

#[derive(Debug)]
struct SectionCopyData {
    pos: usize,
    count: usize,
    temp_name: String,
    fn_desc: String,
}

#[derive(Default, Debug)]
struct PrevLocs {
    text: u32,
    data: u32,
    rodata: u32,
    bss: u32,
}

impl PrevLocs {
    fn get(&self, section: &str) -> u32 {
        match section {
            ".text" => self.text,
            ".data" => self.data,
            ".rodata" => self.rodata,
            ".bss" => self.bss,
            _ => 0,
        }
    }

    fn set(&mut self, section: &str, value: u32) {
        match section {
            ".text" => self.text = value,
            ".data" => self.data = value,
            ".rodata" => self.rodata = value,
            ".bss" => self.bss = value,
            _ => {},
        }
    }
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
/// * `Result<(), ObjFileError>` - Success or error
pub fn fixup_objfile(
    objfile_path: &Path,
    functions: &[Function],
    asm_prelude: &[u8],
    assembler: &str,
    output_enc: &str,
    drop_mdebug_gptab: bool,
    convert_statics: &str,
) -> Result<()> {
    // Read the object file
    let mut objfile = ElfFile::from_file(objfile_path)?;
    let fmt = objfile.fmt.clone();

    let mut prev_locs = PrevLocs::default();
    let mut to_copy: HashMap<String, Vec<SectionCopyData>> = SECTIONS.iter()
        .map(|&s| (s.to_string(), Vec::new()))
        .collect();
    
    let mut asm = Vec::new();
    let mut all_text_glabels = HashSet::new();
    let mut func_sizes = HashMap::new();
    let mut all_late_rodata_dummy_bytes = Vec::new();
    let mut all_jtbl_rodata_size = Vec::new();
    let mut late_rodata_asm = Vec::new();
    let mut modified_text_positions: HashSet<usize> = HashSet::new();
    let mut jtbl_rodata_positions: HashSet<usize> = HashSet::new();
    let mut moved_late_rodata: HashMap<usize, usize> = HashMap::new();
    
    // Process each function
    for function in functions {
        let mut ifdefed = false;
        
        // Check and collect section data
        for (sectype, (temp_name, size)) in &function.data {
            if let Some(temp_name) = temp_name.as_ref() {
                if *size == 0 {
                    continue;
                }
                
                let loc = objfile.find_symbol(temp_name)
                    .ok_or_else(|| ObjFileError::SymbolError(format!("Symbol not found: {}", temp_name)))?;
                let prev_loc = prev_locs.get(sectype);
                
                let prev_loc_usize = u32_to_usize(prev_locs.get(sectype))?;
                let loc_usize = u32_to_usize(loc.1)?;
                
                if loc_usize < prev_loc_usize {
                    return Err(ObjFileError::SectionError(format!(
                        "Incorrectly computed position for section {}", sectype)));
                }
                
                if loc.1 != prev_loc {
                    asm.push(format!(".section {}", sectype));
                    if sectype == ".text" {
                        let nops = ((loc.1 - prev_loc) / 4) as usize;
                        for _ in 0..nops {
                            asm.push("nop".to_string());
                        }
                    } else {
                        asm.push(format!(".space {}", loc.1 - prev_loc));
                    }
                }
                
                to_copy.get_mut(&sectype.to_string()).ok_or_else(|| {
                    ObjFileError::SectionError(format!("Invalid section type: {}", sectype))
                })?.push(SectionCopyData {
                    pos: loc.1 as usize,
                    count: *size,
                    temp_name: temp_name.clone(),
                    fn_desc: function.fn_desc.clone(),
                });
                
                if !function.text_glabels.is_empty() && sectype == ".text" {
                    func_sizes.insert(function.text_glabels[0].clone(), *size);
                }
                
                let size_u32: u32 = (*size).try_into().map_err(|_| 
                    ObjFileError::ConversionError("size conversion failed".to_string()))?;
                prev_locs.set(sectype, loc.1 + size_u32);
            }
        }
        
        if !ifdefed {
            all_text_glabels.extend(function.text_glabels.iter().cloned());
            all_late_rodata_dummy_bytes.push(function.late_rodata_dummy_bytes.clone());
            all_jtbl_rodata_size.push(function.jtbl_rodata_size);
            late_rodata_asm.push(function.late_rodata_asm_conts.clone());
            
            // Add section labels and assembly
            for (sectype, (temp_name, _)) in &function.data {
                if let Some(temp_name) = temp_name {
                    asm.push(format!(".section {}", sectype));
                    asm.push(format!("glabel {}_asm_start", temp_name));
                }
            }
            
            asm.push(".text".to_string());
            asm.extend(function.asm_conts.iter().cloned());
            
            for (sectype, (temp_name, _)) in &function.data {
                if let Some(temp_name) = temp_name {
                    asm.push(format!(".section {}", sectype));
                    asm.push(format!("glabel {}_asm_end", temp_name));
                }
            }
        }
    }

    // Handle late rodata if present
    let mut late_rodata_source_name_start = None;
    let mut late_rodata_source_name_end = None;
    
    if !late_rodata_asm.iter().all(|x| x.is_empty()) {
        late_rodata_source_name_start = Some("_asmpp_late_rodata_start".to_string());
        late_rodata_source_name_end = Some("_asmpp_late_rodata_end".to_string());
        
        asm.push(".section .late_rodata".to_string());
        asm.push(".word 0, 0".to_string());
        asm.push(format!("glabel {}", late_rodata_source_name_start.as_ref().unwrap()));
        for conts in late_rodata_asm {
            asm.extend(conts);
        }
        asm.push(format!("glabel {}", late_rodata_source_name_end.as_ref().unwrap()));
    }

    // Create temporary assembly file
    let mut temp_asm = NamedTempFile::new()?;
    temp_asm.write_all(asm_prelude)?;
    for line in asm {
        temp_asm.write_all(line.as_bytes())?;
        temp_asm.write_all(b"\n")?;
    }
    
    // Create temporary object file
    let temp_obj = NamedTempFile::new()?;
    let temp_asm_path = temp_asm.path().to_str()
        .ok_or_else(|| ObjFileError::Io(io::Error::new(io::ErrorKind::Other, "Invalid temp asm path")))?;
    let temp_obj_path = temp_obj.path().to_str()
        .ok_or_else(|| ObjFileError::Io(io::Error::new(io::ErrorKind::Other, "Invalid temp obj path")))?;

    // Assemble the temporary file
    let status = std::process::Command::new(assembler)
        .arg(temp_asm_path)
        .arg("-o")
        .arg(temp_obj_path)
        .status()?;

    if !status.success() {
        return Err(ObjFileError::ElfError("Failed to assemble".to_string()));
    }

    // Read assembled object file
    let asm_objfile = ElfFile::from_file(Path::new(temp_obj_path))?;

    // Process late rodata if present
    if let (Some(start_name), Some(end_name)) = (late_rodata_source_name_start, late_rodata_source_name_end) {
        if let Some(source) = asm_objfile.find_section(".late_rodata") {
            let start_pos = asm_objfile.find_symbol_in_section(&start_name, source)?;
            let end_pos = asm_objfile.find_symbol_in_section(&end_name, source)?;
            
            let mut pos = start_pos;
            for (dummy_bytes_list, jtbl_size) in all_late_rodata_dummy_bytes.iter().zip(all_jtbl_rodata_size.iter()) {
                for (index, dummy_bytes) in dummy_bytes_list.iter().enumerate() {
                    let mut bytes = Vec::from(*dummy_bytes);
                    bytes.reverse();
                    
                    if let Some(target) = objfile.find_section(".rodata") {
                        if let Some(found_pos) = find_bytes_in_section(&target.data, &bytes, prev_locs.rodata) {
                            // Handle double alignment for non-matching builds
                            if index == 0 && dummy_bytes_list.len() > 1 && 
                               target.data[found_pos+4..found_pos+8] == [0, 0, 0, 0] {
                                // Skip alignment padding
                                pos += 4;
                                continue;
                            }
                            moved_late_rodata.insert(pos, found_pos as u32);
                            pos += 4;
                        }
                    }
                }
                
                if *jtbl_size > 0 {
                    assert!(!dummy_bytes_list.is_empty(), "should always have dummy bytes before jtbl data");
                    let pos_u = pos as usize;
                    for i in 0..*jtbl_size {
                        moved_late_rodata.insert(pos + i as u32, (prev_locs.rodata + i) as u32);
                        jtbl_rodata_positions.insert(prev_locs.rodata + i);
                    }
                    pos += *jtbl_size as u32;
                }
            }
        }
    }

    // Find relocated symbols
    let mut relocated_symbols = HashSet::new();
    for sectype in SECTIONS.iter().chain(&[".late_rodata"]) {
        for obj in &[&asm_objfile, &objfile] {
            if let Some(sec) = obj.find_section(sectype) {
                for reltab in &sec.relocated_by {
                    for rel in &reltab.relocations {
                        if let Some(sym) = obj.symtab.symbol_entries.get(rel.sym_index) {
                            relocated_symbols.insert(sym.clone());
                        }
                    }
                }
            }
        }
    }

    // Process sections
    process_sections(&mut objfile, &to_copy, &all_text_glabels)?;
    
    // Handle reginfo section merging
    if let Some(target_reginfo) = objfile.find_section(".reginfo") {
        if let Some(source_reginfo) = asm_objfile.find_section(".reginfo") {
            let mut data = target_reginfo.data.clone();
            for i in 0..20 {
                data[i] |= source_reginfo.data[i];
            }
            target_reginfo.data = data;
        }
    }

    // Drop debug sections if requested
    if drop_mdebug_gptab {
        objfile.drop_mdebug_gptab();
    }

    // Process symbols and relocations
    process_symbols(&mut objfile, convert_statics, &all_text_glabels, &relocated_symbols, &func_sizes, &moved_late_rodata)?;
    process_relocations(&mut objfile, &modified_text_positions, &jtbl_rodata_positions, &moved_late_rodata)?;

    // Write back the modified object file
    objfile.write(objfile_path.to_str()
        .ok_or_else(|| ObjFileError::Io(io::Error::new(io::ErrorKind::Other, "Invalid output path")))?)
        .map_err(|e| ObjFileError::from(e))?;

    Ok(())
}

fn find_bytes_in_section(data: &[u8], pattern: &[u8], start_pos: usize) -> Option<usize> {
    data[start_pos..].windows(pattern.len())
        .position(|window| window == pattern)
        .map(|pos| pos + start_pos)
}

// Helper functions for type conversions
fn u32_to_usize(val: u32) -> Result<usize> {
    usize::try_from(val).map_err(|_| 
        ObjFileError::ConversionError("u32 to usize conversion failed".to_string()))
}

fn usize_to_u32(val: usize) -> Result<u32> {
    u32::try_from(val).map_err(|_| 
        ObjFileError::ConversionError("usize to u32 conversion failed".to_string()))
}

/// Helper functions for processing different parts of the object file
fn process_sections(
    objfile: &mut ElfFile,
    to_copy: &HashMap<String, Vec<SectionCopyData>>,
    _all_text_glabels: &HashSet<String>,
) -> Result<()> {
    for sectype in SECTIONS {
        let sectype = sectype.to_string();
        if to_copy[&sectype].is_empty() {
            continue;
        }

        let source = objfile.find_section(sectype.as_str())
            .ok_or_else(|| ObjFileError::SectionError(format!("Section not found: {}", sectype)))?;

        // Skip .bss section as it contains no data
        if sectype == ".bss" {
            continue;
        }

        let target = objfile.find_section(sectype.as_str())
            .ok_or_else(|| ObjFileError::SectionError(format!("Target section not found: {}", sectype)))?;

        let mut data = target.data.clone();
        for copy_data in &to_copy[&sectype] {
            let start_sym = format!("{}_asm_start", copy_data.temp_name);
            let end_sym = format!("{}_asm_end", copy_data.temp_name);

            let loc1 = objfile.find_symbol_in_section(&start_sym, source)?;
            let loc2 = objfile.find_symbol_in_section(&end_sym, source)?;

            if loc2 - loc1 != usize_to_u32(copy_data.count)? {
                return Err(ObjFileError::SectionError(
                    format!("Incorrectly computed size for section {}, {}", sectype, copy_data.fn_desc)
                ));
            }

            let start = u32_to_usize(loc1)?;
            let end = u32_to_usize(loc2)?;
            data[copy_data.pos..copy_data.pos + copy_data.count]
                .copy_from_slice(&source.data[start..end]);
        }

        // Update section data
        if let Some(section) = objfile.find_section_mut(sectype.as_str()) {
            section.data = data;
        }
    }

    Ok(())
}

/// Process symbols from .mdebug section
fn process_mdebug_symbols(
    objfile: &mut ElfFile,
    convert_statics: &str,
    objfile_name: &str,
) -> Result<Vec<Symbol>> {
    let mut new_syms = Vec::new();
    
    if let Some(mdebug_section) = objfile.find_section(".mdebug") {
        let mut static_name_count = HashMap::new();
        let mut strtab_index = objfile.symtab.strtab.len();
        let mut new_strtab_data = Vec::new();

        // Extract offsets from mdebug section
        let ifd_max = objfile.fmt.unpack_u32(&mdebug_section.data[18*4..19*4])?;
        let cb_fd_offset = objfile.fmt.unpack_u32(&mdebug_section.data[19*4..20*4])?;
        let cb_sym_offset = objfile.fmt.unpack_u32(&mdebug_section.data[9*4..10*4])?;
        let cb_ss_offset = objfile.fmt.unpack_u32(&mdebug_section.data[15*4..16*4])?;

        // Process each symbol
        for i in 0..ifd_max {
            let offset = (cb_fd_offset + 18*4*i) as usize;
            let iss_base = objfile.fmt.unpack_u32(&mdebug_section.data[offset + 2*4..offset + 3*4])?;
            let isym_base = objfile.fmt.unpack_u32(&mdebug_section.data[offset + 4*4..offset + 5*4])?;
            let csym = objfile.fmt.unpack_u32(&mdebug_section.data[offset + 5*4..offset + 6*4])?;

            let mut scope_level = 0;
            for j in 0..csym {
                let offset2 = (cb_sym_offset + 12 * (isym_base + j)) as usize;
                let iss = objfile.fmt.unpack_u32(&mdebug_section.data[offset2..offset2 + 4])?;
                let value = objfile.fmt.unpack_u32(&mdebug_section.data[offset2 + 4..offset2 + 8])?;
                let st_sc_index = objfile.fmt.unpack_u32(&mdebug_section.data[offset2 + 8..offset2 + 12])?;

                let st = st_sc_index >> 26;
                let sc = (st_sc_index >> 21) & 0x1f;

                // Handle static symbols
                if st == MIPS_DEBUG_ST_STATIC || st == MIPS_DEBUG_ST_STATIC_PROC {
                    let symbol_name_offset = cb_ss_offset + iss_base + iss;
                    let symbol_name = objfile.get_null_terminated_string(symbol_name_offset)?;
                    
                    let mut final_name = symbol_name.clone();
                    if scope_level > 1 {
                        let count = static_name_count.entry(symbol_name.clone())
                            .and_modify(|c| *c += 1)
                            .or_insert(1);
                        final_name = format!("{}:{}", symbol_name, count);
                    }
                    
                    let emitted_name = if convert_statics == "global-with-filename" {
                        format!("{}:{}", objfile_name, final_name)
                    } else {
                        final_name
                    };
                    
                    let section_name = match sc {
                        1 => ".text",
                        2 => ".data",
                        3 => ".bss",
                        15 => ".rodata",
                        _ => continue,
                    };
                    
                    let section = objfile.find_section(section_name)
                        .ok_or_else(|| ObjFileError::SectionError(
                            format!("Section not found: {}", section_name)
                        ))?;
                    
                    let symtype = if sc == 1 { STT_FUNC } else { STT_OBJECT };
                    let binding = if convert_statics == "global" || convert_statics == "global-with-filename" {
                        STB_GLOBAL
                    } else {
                        STB_LOCAL
                    };
                    
                    let sym = Symbol::from_parts(
                        objfile.fmt.clone(),
                        strtab_index,
                        value,
                        0,
                        (binding << 4) | symtype,
                        STV_DEFAULT,
                        section.index as u16,
                        &objfile.sections[objfile.symtab].data,
                        emitted_name.clone(),
                    )?;
                    
                    strtab_index += emitted_name.len() + 1;
                    new_strtab_data.extend_from_slice(emitted_name.as_bytes());
                    new_strtab_data.push(0);
                    new_syms.push(sym);
                }

                // Update scope level
                match st {
                    MIPS_DEBUG_ST_FILE | MIPS_DEBUG_ST_STRUCT | MIPS_DEBUG_ST_UNION |
                    MIPS_DEBUG_ST_ENUM | MIPS_DEBUG_ST_BLOCK | MIPS_DEBUG_ST_PROC |
                    MIPS_DEBUG_ST_STATIC_PROC => scope_level += 1,
                    MIPS_DEBUG_ST_END => scope_level -= 1,
                    _ => {}
                }
            }
            assert_eq!(scope_level, 0);
        }

        objfile.symtab.strtab.extend(&new_strtab_data);
    }

    Ok(new_syms)
}

fn process_symbols(
    objfile: &mut ElfFile,
    convert_statics: &str,
    all_text_glabels: &HashSet<String>,
    relocated_symbols: &HashSet<Symbol>,
    func_sizes: &HashMap<String, usize>,
    moved_late_rodata: &HashMap<usize, usize>,
) -> Result<()> {
    if let Some(symtab) = objfile.find_section_mut(".symtab") {
        let mut new_syms = Vec::new();
        
        // Process existing symbols
        for symbol in &symtab.symbol_entries {
            if !is_temp_name(&symbol.name) {
                new_syms.push(symbol.clone());
            }
        }

        // Sort symbols
        new_syms.sort_by_key(|s| (!s.bind() == STB_LOCAL, s.name.clone() == "_gp_disp"));

        // Update symbol table
        symtab.data = new_syms.iter()
            .flat_map(|s| s.to_bin().unwrap_or_default())
            .collect();
        symtab.sh_info = new_syms.iter()
            .filter(|s| s.bind() == STB_LOCAL)
            .count() as u32;
    }

    Ok(())
}

fn process_relocations(
    objfile: &mut ElfFile,
    modified_text_positions: &HashSet<usize>,
    jtbl_rodata_positions: &HashSet<usize>,
    moved_late_rodata: &HashMap<usize, usize>,
) -> Result<()> {
    for section in &mut objfile.sections {
        if section.sh_type != SHT_REL && section.sh_type != SHT_RELA {
            continue;
        }

        let target_section = objfile.sections.get(section.sh_info as usize)
            .ok_or_else(|| ObjFileError::SectionError("Invalid relocation target section".to_string()))?;

        let mut relocs = section.relocations.clone();
        relocs.retain(|rel| {
            let offset = u32_to_usize(rel.r_offset).unwrap_or(0);
            !(target_section.name == ".text" && modified_text_positions.contains(&offset) ||
              target_section.name == ".rodata" && jtbl_rodata_positions.contains(&offset))
        });

        // Sort relocations by offset
        relocs.sort_by_key(|rel| rel.r_offset);

        // Update section data
        section.data = relocs.iter()
            .flat_map(|r| r.to_bin().unwrap_or_default())
            .collect();
    }

    Ok(())
}
