# ELF Package API Documentation

This document describes the public API surface of the ELF package and how it maps between Python and Rust implementations.

## Core Types

### ElfFile

The main entry point for working with ELF files.
```rust
// Rust
pub struct ElfFile {
    pub data: Vec<u8>,     // Raw file data
    pub fmt: ElfFormat,    // Format information (endianness)
    pub sections: Vec<ElfSection>,
}

// Methods
impl ElfFile {
    // Create new ElfFile from raw bytes
    // Python: ElfFile(data)
    pub fn new(data: &[u8]) -> Result<Self, Error>;

    // Find section by name
    // Python: find_section(name)
    pub fn find_section(&self, name: &str) -> Option<&ElfSection>;

    // Remove .mdebug and .gptab sections
    // Python: drop_mdebug_gptab()
    pub fn drop_mdebug_gptab(&mut self);

    // Add new section
    // Python: add_section(name, sh_type, sh_flags, sh_link, sh_info, sh_addralign, sh_entsize, data)
    pub fn add_section(&mut self, name: &str, 
        sh_type: u32, sh_flags: u32,
        sh_link: usize, sh_info: u32,
        sh_addralign: u32, sh_entsize: u32,
        data: Vec<u8>) -> &mut ElfSection;

    // Write ELF file to disk
    // Python: write(filename)
    pub fn write(&self, filename: &str) -> Result<(), Error>;

    // Find symbol by name in symbol table
    // Python: symtab.find_symbol(name)
    pub fn find_symbol(&self, name: &str) -> Option<(usize, u32)>;

    // Find symbol by name within a specific section
    // Python: symtab.find_symbol_in_section(name, section)
    pub fn find_symbol_in_section(&self, name: &str, section: &ElfSection) -> u32;
}
```

### ElfSection

Represents an ELF section.

```rust
// Rust
pub struct ElfSection {
    pub name: String,
    pub data: Vec<u8>,
    pub index: usize,
    pub relocated_by: Vec<ElfSection>,
    
    // Section header fields
    pub sh_type: u32,
    pub sh_flags: u32,
    pub sh_link: usize,
    pub sh_info: u32,
    pub sh_addralign: u32,
    pub sh_entsize: u32,
}

// Methods
impl ElfSection {
    // Look up string in string table section
    // Python: lookup_str(offset)
    pub fn lookup_str(&self, offset: u32) -> Result<String, Error>;

    // Initialize symbol table (when sh_type == SHT_SYMTAB)
    pub fn init_symbols(&mut self, sections: &[Section]);
    
    // Initialize relocations (when sh_type == SHT_REL or SHT_RELA)
    pub fn init_relocs(&mut self);
    
    // Get local/global symbols
    pub fn local_symbols(&self) -> Vec<Symbol>;
    pub fn global_symbols(&self) -> Vec<Symbol>;
    
    // MIPS debug specific
    pub fn relocate_mdebug(&mut self, original_offset: u32);

    // Find symbol by name in this section's symbol table
    // Python: find_symbol(name)
    pub fn find_symbol(&self, name: &str) -> Option<(usize, u32)>;

    // Find symbol by name within this section
    // Python: find_symbol_in_section(name, section) 
    pub fn find_symbol_in_section(&self, name: &str, section: &ElfSection) -> u32;

    // Get symbol entries
    // Python: symbol_entries property
    pub fn symbol_entries(&self) -> Vec<Symbol>;
}
```

### Symbol

Represents an ELF symbol.

```rust
// Rust
pub struct Symbol {
    pub st_name: u32,
    pub st_value: u32,
    pub st_size: u32,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
    pub name: String,
}

// Methods
impl Symbol {
    // Create symbol from components
    // Python: Symbol.from_parts(fmt, st_name, st_value, st_size, st_info, st_other, st_shndx, strtab, name)
    pub fn from_parts(fmt: ElfFormat,
        st_name: u32, st_value: u32, st_size: u32,
        st_info: u8, st_other: u8, st_shndx: u16,
        strtab: &ElfSection, name: String) -> Result<Self, Error>;

    // Convert symbol to binary format
    // Python: to_bin()
    pub fn to_bin(&self) -> Vec<u8>;

    // Get symbol binding (local/global/weak)
    // Python: bind property
    pub fn bind(&self) -> u8;

    // Get symbol type (object/func/section/etc)
    // Python: type property
    pub fn type_(&self) -> u8;

    // Replace this symbol with another
    // Python: replace_by property
    pub fn replace_by(&mut self, other: Symbol);

    // Get/set new index for symbol
    // Python: new_index property
    pub fn new_index(&self) -> usize;
    pub fn set_new_index(&mut self, index: usize);

    // Get/set symbol binding (local/global)
    // Python: bind property
    pub fn bind(&self) -> u8;
    pub fn set_bind(&mut self, bind: u8);

    // Get/set symbol type
    // Python: type property 
    pub fn type_(&self) -> u8;
    pub fn set_type(&mut self, type_: u8);
}
```

### Relocation

Represents a relocation entry.

```rust
// Rust
pub struct Relocation {
    pub r_offset: u32,
    pub r_info: u32,
    pub sym_index: usize,
    pub r_addend: Option<u32>,
}

// Methods
impl Relocation {
    // Convert relocation to binary format
    // Python: to_bin()
    pub fn to_bin(&self) -> Vec<u8>;
}
```

## Key Differences from Python Implementation

1. **Error Handling**: 
   - Python: Uses custom `Failure` exceptions
   - Rust: Uses `Result<T, Error>` with custom error types

2. **Memory Management**:
   - Python: Automatic garbage collection
   - Rust: Ownership and borrowing rules

3. **Binary Data Handling**:
   - Python: Uses struct module for packing/unpacking
   - Rust: Custom implementation with endianness support

4. **String Handling**:
   - Python: Unicode strings with encoding/decoding
   - Rust: Explicit UTF-8 String type

## Common Usage Patterns

### Loading and Modifying an ELF File

```rust
// Load ELF file
let data = std::fs::read(path)?;
let mut elf = ElfFile::new(&data)?;

// Find and modify section
if let Some(section) = elf.find_section(".text") {
    // Modify section data
    let mut data = section.data.clone();
    // ... modify data ...
    section.data = data;
}

// Write modified file
elf.write(output_path)?;
```

### Symbol Table Operations

```rust
// Find symbol
if let Some((idx, value)) = elf.symtab.find_symbol("main") {
    // Use symbol index and value
}

// Create new symbol
let symbol = Symbol::from_parts(
    elf.fmt,
    st_name,
    st_value,
    st_size,
    st_info,
    st_other,
    st_shndx,
    &elf.symtab.strtab,
    name.to_string(),
)?;
```

### Adding New Sections

```rust
let section = elf.add_section(
    ".new_section",
    SHT_PROGBITS,
    SHF_ALLOC,
    0,
    0,
    4,
    0,
    Vec::new(),
);
```

## Additional Constants

```rust
// Symbol bindings
pub const STB_LOCAL: u8 = 0;
pub const STB_GLOBAL: u8 = 1;

// Symbol types
pub const STT_OBJECT: u8 = 1;
pub const STT_FUNC: u8 = 2;

// Symbol visibility
pub const STV_DEFAULT: u8 = 0;

// Special section indices
pub const SHN_UNDEF: u16 = 0;
pub const SHN_ABS: u16 = 0xfff1;

// Section types
pub const SHT_REL: u32 = 9;
pub const SHT_RELA: u32 = 4;

// MIPS debug symbol types
pub const MIPS_DEBUG_ST_STATIC: u32 = 7;
pub const MIPS_DEBUG_ST_STATIC_PROC: u32 = 8;
pub const MIPS_DEBUG_ST_FILE: u32 = 0;
pub const MIPS_DEBUG_ST_STRUCT: u32 = 1; 
pub const MIPS_DEBUG_ST_UNION: u32 = 2;
pub const MIPS_DEBUG_ST_ENUM: u32 = 3;
pub const MIPS_DEBUG_ST_BLOCK: u32 = 4;
pub const MIPS_DEBUG_ST_PROC: u32 = 5;
pub const MIPS_DEBUG_ST_END: u32 = 6;
```

