# ELF Package API Documentation

This document describes the public API surface of the ELF package and how it maps between Python and Rust implementations.

## Core Types

### ElfFile

The main entry point for working with ELF files.
```rust
// Rust
pub struct ElfFile {
    pub header: ElfHeader,
    pub sections: Vec<ElfSection>,
    pub fmt: ElfFormat,
    pub symtab: usize,  // Index of symbol table section
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
    pub fn add_section(&mut self, name: &str, sh_type: u32, sh_flags: u32, 
                      sh_link: u32, sh_info: u32, sh_addralign: u32, 
                      sh_entsize: u32, data: Vec<u8>) -> Result<usize, Error>;

    // Write ELF file to disk
    // Python: write(filename)
    pub fn write(&self, filename: &str) -> Result<(), Error>;

    // Find symbol by name in symbol table
    // Python: symtab.find_symbol(name)
    pub fn find_symbol(&self, name: &str) -> Option<(usize, u32)>;

    // Find symbol by name within a specific section
    // Python: symtab.find_symbol_in_section(name, section)
    pub fn find_symbol_in_section(&self, name: &str, section: &ElfSection) -> u32;

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, Error>;
}
```

### ElfSection

Represents an ELF section.

```rust
// Rust
pub struct ElfSection {
    pub fmt: ElfFormat,
    pub sh_name: u32,
    pub sh_type: u32,
    pub sh_flags: u32,
    pub sh_addr: u32,
    pub sh_offset: u32,
    pub sh_size: u32,
    pub sh_link: u32,
    pub sh_info: u32,
    pub sh_addralign: u32,
    pub sh_entsize: u32,
    pub data: Vec<u8>,
    pub symbols: Vec<Symbol>,
    pub relocations: Vec<Relocation>,
    pub index: usize,
    pub name: String,
    pub relocated_by: Vec<usize>,
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
    pub fn find_symbol_in_section(&self, name: &str, section: &ElfSection) -> Result<u32, Error> {

    // Get symbol entries
    // Python: symbol_entries property
    pub fn symbol_entries(&self) -> Vec<Symbol>;

    pub fn new(fmt: ElfFormat, header: &[u8]) -> Result<Self, Error>;
    pub fn init_data(&mut self, data: &[u8]) -> Result<(), Error>;
    pub fn late_init(&mut self, sections: &mut [ElfSection]) -> Result<(), Error>;
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
    pub visibility: u8,
    pub fmt: ElfFormat,
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
// ELF Header constants
pub const EI_NIDENT: usize = 16;
pub const EI_CLASS: usize = 4;
pub const EI_DATA: usize = 5;
pub const EI_VERSION: usize = 6;
pub const EI_OSABI: usize = 7;
pub const EI_ABIVERSION: usize = 8;
pub const STN_UNDEF: u32 = 0;

// Section Header constants
pub const SHN_UNDEF: u16 = 0;
pub const SHN_ABS: u16 = 0xfff1;
pub const SHN_COMMON: u16 = 0xfff2;
pub const SHN_XINDEX: u16 = 0xffff;
pub const SHN_LORESERVE: u16 = 0xff00;

// Symbol Type constants
pub const STT_NOTYPE: u8 = 0;
pub const STT_OBJECT: u8 = 1;
pub const STT_FUNC: u8 = 2;
pub const STT_SECTION: u8 = 3;
pub const STT_FILE: u8 = 4;
pub const STT_COMMON: u8 = 5;
pub const STT_TLS: u8 = 6;

// Symbol Binding constants
pub const STB_LOCAL: u8 = 0;
pub const STB_GLOBAL: u8 = 1;
pub const STB_WEAK: u8 = 2;

// Symbol Visibility constants
pub const STV_DEFAULT: u8 = 0;
pub const STV_INTERNAL: u8 = 1;
pub const STV_HIDDEN: u8 = 2;
pub const STV_PROTECTED: u8 = 3;

// Section Header Type constants
pub const SHT_NULL: u32 = 0;
pub const SHT_PROGBITS: u32 = 1;
pub const SHT_SYMTAB: u32 = 2;
pub const SHT_STRTAB: u32 = 3;
pub const SHT_RELA: u32 = 4;
pub const SHT_HASH: u32 = 5;
pub const SHT_DYNAMIC: u32 = 6;
pub const SHT_NOTE: u32 = 7;
pub const SHT_NOBITS: u32 = 8;
pub const SHT_REL: u32 = 9;
pub const SHT_SHLIB: u32 = 10;
pub const SHT_DYNSYM: u32 = 11;
pub const SHT_INIT_ARRAY: u32 = 14;
pub const SHT_FINI_ARRAY: u32 = 15;
pub const SHT_PREINIT_ARRAY: u32 = 16;
pub const SHT_GROUP: u32 = 17;
pub const SHT_SYMTAB_SHNDX: u32 = 18;
pub const SHT_MIPS_GPTAB: u32 = 0x70000003;
pub const SHT_MIPS_DEBUG: u32 = 0x70000005;
pub const SHT_MIPS_REGINFO: u32 = 0x70000006;
pub const SHT_MIPS_OPTIONS: u32 = 0x7000000d;

// Section Header Flags
pub const SHF_WRITE: u32 = 0x1;
pub const SHF_ALLOC: u32 = 0x2;
pub const SHF_EXECINSTR: u32 = 0x4;
pub const SHF_MERGE: u32 = 0x10;
pub const SHF_STRINGS: u32 = 0x20;
pub const SHF_INFO_LINK: u32 = 0x40;
pub const SHF_LINK_ORDER: u32 = 0x80;
pub const SHF_OS_NONCONFORMING: u32 = 0x100;
pub const SHF_GROUP: u32 = 0x200;
pub const SHF_TLS: u32 = 0x400;
```

### ElfFormat

Represents the format of an ELF file.

```rust
pub struct ElfFormat {
    pub is_big_endian: bool,
}

impl ElfFormat {
    pub fn new(is_big_endian: bool) -> Self;
    pub fn pack_u16(&self, data: &mut [u8], value: u16) -> Result<(), Error>;
    pub fn pack_u32(&self, data: &mut [u8], value: u32) -> Result<(), Error>;
    pub fn unpack_u16(&self, data: &[u8]) -> Result<u16, Error>;
    pub fn unpack_u32(&self, data: &[u8]) -> Result<u32, Error>;
    pub fn pack_symbol(&self, symbol: &Symbol) -> Result<Vec<u8>, Error>;
    pub fn unpack_symbol(&self, data: &[u8]) -> Result<(u32, u32, u32, u8, u8, u16), Error>;
}
```

### ElfHeader

Represents the ELF header.

```rust
pub struct ElfHeader {
    pub e_ident: [u8; EI_NIDENT],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u32,
    pub e_phoff: u32,
    pub e_shoff: u32,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}
```

## MIPS-Specific Features

### Debug Sections
- Support for `.mdebug` sections (SHT_MIPS_DEBUG)
- Support for `.gptab` sections (SHT_MIPS_GPTAB)
- Relocation of debug information
- MIPS-specific section types and flags

### Methods
```rust
impl ElfFile {
    // Remove MIPS debug sections
    pub fn drop_mdebug_gptab(&mut self);
}

impl ElfSection {
    // Relocate MIPS debug information
    pub fn relocate_mdebug(&mut self, original_offset: u32) -> Result<(), Error>;
}
```

## Error Handling

The library uses a custom Error type that covers:
- Invalid format errors
- I/O errors
- Data parsing errors
- Section manipulation errors
- Symbol resolution errors

All operations that can fail return `Result<T, Error>`.

pub trait Section {
    fn lookup_str(&self, index: usize) -> Result<String, Error>;
    fn add_str(&mut self, s: &str) -> Result<u32, Error>;
}

