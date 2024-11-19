# ASM Processor Migration Plan

This document tracks the migration of the ASM Processor from Python to Rust.

## Migration Strategy

1. **Code Restructuring**
   - Split Python codebase into logical modules while keeping code changes minimal
   - Create a comparable Rust architecture
   - Maintain close parity with Python implementation

2. **Migration Process**
   - Validate existing codebase with run-tests.sh
   - Document architectural decisions and data types
   - Initialize Rust project
   - Convert Python modules to Rust incrementally
   - Fix build errors
   - Verify functionality via run-tests.sh
   - Remove Python-specific constructs

## 1. Python Architecture

The current codebase will be split into modules while keeping actual code changes to a minimum. Only import statements will be modified.

### Directory Structure

```
processor/
├── elf/
│   ├── __init__.py       # Exports all ELF-related classes
│   ├── format.py         # ElfFormat: Handles big/little endian formatting
│   ├── header.py         # ElfHeader: ELF file header structure
│   ├── section.py        # Section: Section header and data
│   ├── symbol.py         # Symbol: Symbol table entries
│   ├── relocation.py     # Relocation: Relocation entries
│   └── file.py          # ElfFile: Main ELF file handling
├── asm/
│   ├── __init__.py       # Exports ASM-related functionality
│   ├── block.py          # GlobalAsmBlock: Assembly block processing
│   └── function.py       # Function namedtuple and related code
├── utils/
│   ├── __init__.py       # Exports utilities
│   ├── state.py          # GlobalState: Manages processing state
│   └── errors.py         # Failure: Error handling
├── processor.py         # parse_source: Main source processing logic
├── objfile.py          # fixup_objfile: Object file processing
└── __init__.py           # Main entry point with run() function

### Key Components

1. **ELF Processing (`elf/`)**
   - Core ELF file manipulation
   - Section and symbol management
   - Relocation handling
   - Endianness support

2. **Assembly Processing (`asm/`)**
   - Assembly block parsing
   - Function handling
   - Source processing

3. **Utilities (`utils/`)**
   - Global state management
   - Error handling
   - Common functionality

### Code Organization Rules

1. **Minimal Changes**
   - Keep all existing code functionality identical
   - Only modify import statements
   - Maintain all class and function names
   - Keep all constants in their original location

2. **Module Structure**
   - Each class moves to its own file
   - __init__.py files re-export all public items
   - Main entry point remains in __init__.py

3. **Dependencies**
   - Original dependencies remain unchanged
   - No new external dependencies will be added

## 2. Rust Architecture

The Rust implementation will closely mirror the Python structure while adopting Rust idioms where appropriate.

### Project Structure

```rust
src/
├── elf/
│   ├── mod.rs           // Re-exports ELF modules
│   ├── format.rs        // ElfFormat: Endianness handling
│   ├── header.rs        // ElfHeader: File header structs
│   ├── section.rs       // Section: Section handling
│   ├── symbol.rs        // Symbol: Symbol table entries
│   ├── relocation.rs    // Relocation: Relocation entries
│   └── file.rs         // ElfFile: Main ELF processing
├── asm/
│   ├── mod.rs           // Re-exports ASM modules
│   ├── block.rs         // GlobalAsmBlock: Assembly block processing
│   └── function.rs      // Function: Assembly function handling
├── utils/
│   ├── mod.rs           // Re-exports utility modules
│   ├── state.rs         // GlobalState: Processing state
│   ├── error.rs         // Error: Error handling
│   └── constants.rs     // Constants: Shared constants
├── processor.rs        // parse_source: Main source processing
├── objfile.rs         // fixup_objfile: Object file processing
└── lib.rs              // Library entry point

### Key Components

1. **ELF Processing (`elf/`)**
   - Core ELF file manipulation
   - Section and symbol management
   - Relocation handling
   - Endianness support

2. **Assembly Processing (`asm/`)**
   - Assembly block parsing and processing
   - Function structure and management
   - Late rodata handling
   - Section size tracking
   - Assembly directive processing

3. **Utilities (`utils/`)**
   - Global state management
   - Error handling with custom Error types
   - Shared constants
   - Common functionality

### Code Organization Rules

1. **Rust Idioms**
   - Use Result for error handling
   - Implement proper traits (Debug, Clone)
   - Strong type safety with proper validation
   - Memory safety with ownership rules

2. **Module Structure**
   - Each component in its own module
   - Clear module hierarchy
   - Proper visibility rules
   - Re-exports through mod.rs

3. **Dependencies**
   - Minimal external dependencies
   - Use of lazy_static for regex
   - Proper error handling traits

### Key Data Structures

1. **ELF Processing**
```rust
// elf/format.rs
pub struct ElfFormat {
    pub is_big_endian: bool,
}

// elf/header.rs
pub struct ElfHeader {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    // ... other fields
}

// elf/section.rs
pub struct Section {
    pub name: String,
    pub header: SectionHeader,
    pub data: Vec<u8>,
}

// elf/symbol.rs
pub struct Symbol {
    pub name: String,
    pub value: u32,
    pub size: u32,
    pub info: u8,
    pub other: u8,
    pub shndx: u16,
}

// elf/file.rs
pub struct ElfFile {
    pub header: ElfHeader,
    pub sections: Vec<Section>,
    pub symbols: Vec<Symbol>,
    pub relocations: Vec<Relocation>,
}
```

2. **Assembly Processing**
```rust
// asm/function.rs
pub struct Function {
    pub text_size: usize,
    pub data_size: usize,
    pub rodata_size: usize,
    pub bss_size: usize,
    pub late_rodata_size: usize,
}

// asm/block.rs
pub struct GlobalAsmBlock {
    pub fn_desc: String,
    pub cur_section: String,
    pub asm_conts: Vec<String>,
    pub late_rodata_asm_conts: Vec<String>,
    pub text_glabels: Vec<String>,
    pub fn_section_sizes: HashMap<String, usize>,
}
```

3. **State and Error Handling**
```rust
// utils/state.rs
pub struct GlobalState {
    pub min_instr_count: usize,
    pub skip_instr_count: usize,
    pub use_jtbl_for_rodata: bool,
    pub prelude_if_late_rodata: usize,
    pub mips1: bool,
    pub pascal: bool,
}

// utils/error.rs
#[derive(Debug)]
pub enum Error {
    IoError(std::io::Error),
    ParseError(String),
    ProcessingError(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

### Python-isms and Compatibility

1. **String Handling**
   - Python's flexible string handling will be replaced with explicit UTF-8/Latin1 encoding
   - String operations will use the `encoding_rs` crate for compatibility
   ```rust
   use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
   ```

2. **Regular Expressions**
   - Port Python regex patterns to Rust's regex syntax
   - Use the `regex` crate with similar capture group handling
   ```rust
   use regex::Regex;
   
   lazy_static! {
       static ref RE_COMMENT_OR_STRING: Regex = Regex::new(
           r#"#.*|/\*.*?\*/|"(?:\\.|[^\\"])*""#
       ).unwrap();
   }
   ```

3. **File Operations**
   - Replace Python's flexible file handling with Rust's `std::fs` and `std::io`
   - Implement buffered reading for efficiency
   ```rust
   use std::fs::File;
   use std::io::{BufReader, BufWriter};
   ```

4. **Dynamic Typing**
   - Replace Python's dynamic typing with static types and enums
   - Use `Option` and `Result` for error handling
   ```rust
   #[derive(Debug)]
   pub enum SectionContent {
       Text(Vec<u8>),
       Data(Vec<u8>),
       Bss(usize),
       Rodata(Vec<u8>),
   }
   ```

### Build System

1. **Cargo.toml Dependencies**
```toml
[package]
name = "asm-processor"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4.0", features = ["derive"] }
regex = "1.0"
encoding_rs = "0.8"
lazy_static = "1.4"
thiserror = "1.0"
byteorder = "1.4"
```

2. **CLI Interface**
   - Use `clap` for argument parsing to match Python's argparse
   - Maintain exact same command-line interface
   ```rust
   use clap::Parser;
   
   #[derive(Parser)]
   #[command(about = "Pre-process .c files and post-process .o files")]
   struct Opts {
       #[arg(help = "path to .c code")]
       filename: String,
       
       #[arg(long, help = "path to .o file to post-process")]
       post_process: Option<String>,
       
       // ... other options
   }
   ```

### Testing Strategy

1. **Test Coverage**
   - Port all Python tests to Rust
   - Use similar test data and fixtures
   - Add Rust-specific unit tests

2. **Integration Tests**
   - Maintain compatibility with `run-tests.sh`
   - Add Rust-specific integration tests
   - Compare output with Python version

3. **Property Tests**
   - Add QuickCheck-style property tests for core functionality
   - Focus on ELF parsing and assembly processing

### Migration Status

### Completed
- [x] Basic project structure
- [x] Error handling module
- [x] ELF format module
- [x] ELF header module
- [x] ELF section module
- [x] ELF symbol module
- [x] ELF relocation module
- [x] Constants module

### In Progress
- [ ] ELF file module
- [ ] ASM block module
- [ ] Function module
- [ ] State module
- [ ] Processor module
- [ ] Object file module

### Pending
- [ ] Integration tests
- [ ] Performance optimizations
- [ ] Documentation
- [ ] CI/CD setup

### Notes
- All ELF-related core modules have been migrated with proper error handling
- Using Rust's type system for improved safety
- Constants consolidated in a single module
- Test coverage maintained for migrated modules

### Migration Steps

1. **Phase 1: Core Infrastructure**
   - Set up Rust project structure
   - Implement error types and utilities
   - Port ELF parsing infrastructure

2. **Phase 2: Assembly Processing**
   - Implement assembly block parsing
   - Port function handling
   - Add text processing utilities

3. **Phase 3: Main Logic**
   - Implement source processing
   - Port object file handling
   - Add CLI interface
