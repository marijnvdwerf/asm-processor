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

// MIPS Relocation Types
pub const R_MIPS_32: u32 = 2;
pub const R_MIPS_26: u32 = 4;
pub const R_MIPS_HI16: u32 = 5;
pub const R_MIPS_LO16: u32 = 6;

// MIPS Debug Constants
pub const MIPS_DEBUG_ST_STATIC: u32 = 2;
pub const MIPS_DEBUG_ST_PROC: u32 = 6;
pub const MIPS_DEBUG_ST_BLOCK: u32 = 7;
pub const MIPS_DEBUG_ST_END: u32 = 8;
pub const MIPS_DEBUG_ST_FILE: u32 = 11;
pub const MIPS_DEBUG_ST_STATIC_PROC: u32 = 14;
pub const MIPS_DEBUG_ST_STRUCT: u32 = 26;
pub const MIPS_DEBUG_ST_UNION: u32 = 27;
pub const MIPS_DEBUG_ST_ENUM: u32 = 28;
