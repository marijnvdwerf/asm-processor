use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::Path;
use tempfile::NamedTempFile;

use crate::elf::{
    Symbol,
    constants::{
        STB_LOCAL, SHT_REL, SHT_RELA
    }
};

use crate::elf::file::ElfFile;
use crate::utils::Error as CrateError;
use crate::asm::Function;
use crate::elf::Relocation;
use crate::elf::section::ElfSection;

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
    _output_enc: &str,
    drop_mdebug_gptab: bool,
    convert_statics: &str,
) -> Result<()> {
    // Read the object file
    let mut objfile = ElfFile::from_file(objfile_path)?;
    let _fmt = objfile.fmt.clone();

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
    let modified_text_positions: HashSet<usize> = HashSet::new();
    let mut jtbl_rodata_positions: HashSet<usize> = HashSet::new();
    let mut moved_late_rodata: HashMap<usize, usize> = HashMap::new();
    
    // Process each function
    for function in functions {
        let ifdefed = false;
        
        // Process each section data
        for (sectype, data_tuple) in &function.data {
            let temp_name = &data_tuple.0;
            let size = data_tuple.1;
            
            if !temp_name.is_empty() {
                if size == 0 {
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
                    count: size,
                    temp_name: temp_name.to_string(),
                    fn_desc: function.fn_desc.clone(),
                });
                
                if !function.text_glabels.is_empty() && sectype == ".text" {
                    func_sizes.insert(function.text_glabels[0].clone(), size);
                }
                
                let size_u32: u32 = (size).try_into().map_err(|_| 
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
            for (sectype, data_tuple) in &function.data {
                let temp_name = &data_tuple.0;
                if !temp_name.is_empty() {
                    asm.push(format!(".section {}", sectype));
                    asm.push(format!("glabel {}_asm_start", temp_name));
                }
            }
            
            asm.push(".text".to_string());
            asm.extend(function.asm_conts.iter().cloned());
            
            for (sectype, data_tuple) in &function.data {
                let temp_name = &data_tuple.0;
                if !temp_name.is_empty() {
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
            let _end_pos = asm_objfile.find_symbol_in_section(&end_name, source)?;
            
            let mut pos = start_pos;
            for (dummy_bytes_list, jtbl_size) in all_late_rodata_dummy_bytes.iter().zip(all_jtbl_rodata_size.iter()) {
                for (index, dummy_bytes) in dummy_bytes_list.iter().enumerate() {
                    let mut bytes = Vec::from(dummy_bytes.as_bytes());
                    bytes.reverse();
                    
                    if let Some(target) = objfile.find_section(".rodata") {
                        if let Some(found_pos) = find_bytes_in_section(&target.data, &bytes, prev_locs.rodata as usize) {
                            // Handle double alignment for non-matching builds
                            if index == 0 && dummy_bytes_list.len() > 1 && 
                               target.data[found_pos+4..found_pos+8] == [0, 0, 0, 0] {
                                // Skip alignment padding
                                pos += 4;
                                continue;
                            }
                            moved_late_rodata.insert(pos as usize, found_pos);
                            pos += 4;
                        }
                    }
                }
                
                if *jtbl_size > 0 {
                    assert!(!dummy_bytes_list.is_empty(), "should always have dummy bytes before jtbl data");
                    let pos_usize = pos as usize;
                    for i in 0..*jtbl_size {
                        moved_late_rodata.insert(pos_usize + i, prev_locs.rodata as usize + i);
                        jtbl_rodata_positions.insert(prev_locs.rodata as usize + i);
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
                    if let Some(reltab_section) = objfile.find_section(&reltab.to_string()) {
                        for rel in &reltab_section.relocations {
                            if let Some(sym) = objfile.get_symbol_entries().get(rel.get_sym_index() as usize) {
                                relocated_symbols.insert(sym.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // Process sections
    process_sections(&mut objfile, &to_copy, &all_text_glabels)?;
    
    // Handle reginfo section merging
    if let Some(target_reginfo) = objfile.find_section_mut(".reginfo") {
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

fn process_symbols(
    objfile: &mut ElfFile,
    _convert_statics: &str,
    _all_text_glabels: &HashSet<String>,
    _relocated_symbols: &HashSet<Symbol>,
    _func_sizes: &HashMap<String, usize>,
    _moved_late_rodata: &HashMap<usize, usize>,
) -> Result<()> {
    if let Some(symtab) = objfile.find_section_mut(".symtab") {
        let mut new_syms = Vec::new();
        
        // Process existing symbols
        for symbol in &symtab.symbols {
            if !is_temp_name(&symbol.name) {
                new_syms.push(symbol.clone());
            }
        }

        // Sort symbols
        new_syms.sort_by_key(|s| (!s.bind() == STB_LOCAL, s.name.clone() == "_gp_disp"));

        let local_count = new_syms.iter()
            .filter(|s| s.bind() == STB_LOCAL)
            .count() as u32;

        symtab.symbols = new_syms;
        symtab.sh_info = local_count;
    }

    Ok(())
}

fn process_relocations(
    objfile: &mut ElfFile,
    modified_text_positions: &HashSet<usize>,
    jtbl_rodata_positions: &HashSet<usize>,
    _moved_late_rodata: &HashMap<usize, usize>,
) -> Result<()> {
    let mut sections_to_process = Vec::new();
    
    // Collect sections to process first
    for section in &objfile.sections {
        if section.sh_type != SHT_REL && section.sh_type != SHT_RELA {
            continue;
        }
        sections_to_process.push((section.index, section.sh_info));
    }

    // Process each section
    for (section_idx, target_idx) in sections_to_process {
        // Get target section info first
        let target_name = {
            let target_section = &objfile.sections[target_idx as usize];
            target_section.name.clone()
        };
        
        // Then process the relocation section
        let section = &mut objfile.sections[section_idx];
        
        let mut relocs = section.relocations.clone();
        relocs.retain(|rel| {
            let offset = u32_to_usize(rel.r_offset).unwrap_or(0);
            !(target_name == ".text" && modified_text_positions.contains(&offset) ||
              target_name == ".rodata" && jtbl_rodata_positions.contains(&offset))
        });

        relocs.sort_by_key(|rel| rel.r_offset);
        section.data = relocs.iter()
            .flat_map(|r| r.to_bytes(&section.fmt))
            .collect();
    }

    Ok(())
}

impl ElfFile {
    fn get_symbol_entries(&self) -> &Vec<Symbol> {
        &self.sections[self.symtab].symbols
    }

    fn find_section_mut(&mut self, name: &str) -> Option<&mut ElfSection> {
        self.sections.iter_mut().find(|s| s.name == name)
    }

}

impl Relocation {
    fn get_sym_index(&self) -> u32 {
        self.r_info >> 8
    }
}

impl Hash for Symbol {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.st_value.hash(state);
        self.st_size.hash(state);
        self.st_info.hash(state);
        self.st_other.hash(state);
        self.st_shndx.hash(state);
    }
}

impl PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name &&
        self.st_value == other.st_value &&
        self.st_size == other.st_size &&
        self.st_info == other.st_info &&
        self.st_other == other.st_other &&
        self.st_shndx == other.st_shndx
    }
}

impl Eq for Symbol {}
