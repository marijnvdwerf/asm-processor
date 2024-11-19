use crate::elf::constants::*;
use crate::elf::format::ElfFormat;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ElfHeaderError {
    #[error("Invalid ELF class (expected 32-bit)")]
    InvalidClass,
    #[error("Invalid ELF type (expected relocatable)")]
    InvalidType,
    #[error("Invalid machine type (expected MIPS I)")]
    InvalidMachine,
    #[error("Invalid program header offset (expected 0)")]
    InvalidProgramHeaderOffset,
    #[error("Invalid section header offset (expected non-zero)")]
    InvalidSectionHeaderOffset,
    #[error("Invalid section string table index")]
    InvalidSectionStringTableIndex,
}

#[derive(Debug, Clone)]
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
    pub fmt: ElfFormat,
}

impl ElfHeader {
    pub fn new(data: &[u8]) -> Result<Self, ElfHeaderError> {
        let mut e_ident = [0u8; EI_NIDENT];
        e_ident.copy_from_slice(&data[..EI_NIDENT]);
        
        // Verify 32-bit class
        if e_ident[EI_CLASS] != 1 {
            return Err(ElfHeaderError::InvalidClass);
        }

        let fmt = ElfFormat::new(e_ident[EI_DATA] == 2);
        
        // Parse the remaining fields
        let e_type = fmt.unpack_u16(&data[EI_NIDENT..EI_NIDENT + 2]);
        let e_machine = fmt.unpack_u16(&data[EI_NIDENT + 2..EI_NIDENT + 4]);
        let e_version = fmt.unpack_u32(&data[EI_NIDENT + 4..EI_NIDENT + 8]);
        let e_entry = fmt.unpack_u32(&data[EI_NIDENT + 8..EI_NIDENT + 12]);
        let e_phoff = fmt.unpack_u32(&data[EI_NIDENT + 12..EI_NIDENT + 16]);
        let e_shoff = fmt.unpack_u32(&data[EI_NIDENT + 16..EI_NIDENT + 20]);
        let e_flags = fmt.unpack_u32(&data[EI_NIDENT + 20..EI_NIDENT + 24]);
        let e_ehsize = fmt.unpack_u16(&data[EI_NIDENT + 24..EI_NIDENT + 26]);
        let e_phentsize = fmt.unpack_u16(&data[EI_NIDENT + 26..EI_NIDENT + 28]);
        let e_phnum = fmt.unpack_u16(&data[EI_NIDENT + 28..EI_NIDENT + 30]);
        let e_shentsize = fmt.unpack_u16(&data[EI_NIDENT + 30..EI_NIDENT + 32]);
        let e_shnum = fmt.unpack_u16(&data[EI_NIDENT + 32..EI_NIDENT + 34]);
        let e_shstrndx = fmt.unpack_u16(&data[EI_NIDENT + 34..EI_NIDENT + 36]);

        // Validate fields
        if e_type != 1 {
            return Err(ElfHeaderError::InvalidType);
        }
        if e_machine != 8 {
            return Err(ElfHeaderError::InvalidMachine);
        }
        if e_phoff != 0 {
            return Err(ElfHeaderError::InvalidProgramHeaderOffset);
        }
        if e_shoff == 0 {
            return Err(ElfHeaderError::InvalidSectionHeaderOffset);
        }
        if e_shstrndx == SHN_UNDEF {
            return Err(ElfHeaderError::InvalidSectionStringTableIndex);
        }

        Ok(Self {
            e_ident,
            e_type,
            e_machine,
            e_version,
            e_entry,
            e_phoff,
            e_shoff,
            e_flags,
            e_ehsize,
            e_phentsize,
            e_phnum,
            e_shentsize,
            e_shnum,
            e_shstrndx,
            fmt,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(EI_NIDENT + 36);
        result.extend_from_slice(&self.e_ident);
        result.extend_from_slice(&self.fmt.pack_u16(self.e_type));
        result.extend_from_slice(&self.fmt.pack_u16(self.e_machine));
        result.extend_from_slice(&self.fmt.pack_u32(self.e_version));
        result.extend_from_slice(&self.fmt.pack_u32(self.e_entry));
        result.extend_from_slice(&self.fmt.pack_u32(self.e_phoff));
        result.extend_from_slice(&self.fmt.pack_u32(self.e_shoff));
        result.extend_from_slice(&self.fmt.pack_u32(self.e_flags));
        result.extend_from_slice(&self.fmt.pack_u16(self.e_ehsize));
        result.extend_from_slice(&self.fmt.pack_u16(self.e_phentsize));
        result.extend_from_slice(&self.fmt.pack_u16(self.e_phnum));
        result.extend_from_slice(&self.fmt.pack_u16(self.e_shentsize));
        result.extend_from_slice(&self.fmt.pack_u16(self.e_shnum));
        result.extend_from_slice(&self.fmt.pack_u16(self.e_shstrndx));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> Vec<u8> {
        let mut data = vec![0; EI_NIDENT + 36];
        // Set e_ident
        data[EI_CLASS] = 1; // 32-bit
        data[EI_DATA] = 2;  // big-endian
        
        let fmt = ElfFormat::new(true);
        
        // Write header fields
        let offset = EI_NIDENT;
        data[offset..offset + 2].copy_from_slice(&fmt.pack_u16(1)); // e_type (relocatable)
        data[offset + 2..offset + 4].copy_from_slice(&fmt.pack_u16(8)); // e_machine (MIPS I)
        data[offset + 4..offset + 8].copy_from_slice(&fmt.pack_u32(1)); // e_version
        data[offset + 8..offset + 12].copy_from_slice(&fmt.pack_u32(0)); // e_entry
        data[offset + 12..offset + 16].copy_from_slice(&fmt.pack_u32(0)); // e_phoff
        data[offset + 16..offset + 20].copy_from_slice(&fmt.pack_u32(52)); // e_shoff
        data[offset + 20..offset + 24].copy_from_slice(&fmt.pack_u32(0)); // e_flags
        data[offset + 24..offset + 26].copy_from_slice(&fmt.pack_u16(52)); // e_ehsize
        data[offset + 26..offset + 28].copy_from_slice(&fmt.pack_u16(0)); // e_phentsize
        data[offset + 28..offset + 30].copy_from_slice(&fmt.pack_u16(0)); // e_phnum
        data[offset + 30..offset + 32].copy_from_slice(&fmt.pack_u16(40)); // e_shentsize
        data[offset + 32..offset + 34].copy_from_slice(&fmt.pack_u16(3)); // e_shnum
        data[offset + 34..offset + 36].copy_from_slice(&fmt.pack_u16(2)); // e_shstrndx
        
        data
    }

    #[test]
    fn test_elf_header_parse() {
        let data = create_test_data();
        let header = ElfHeader::new(&data).unwrap();
        
        assert_eq!(header.e_type, 1);
        assert_eq!(header.e_machine, 8);
        assert_eq!(header.e_shoff, 52);
        assert_eq!(header.e_shstrndx, 2);
    }

    #[test]
    fn test_elf_header_roundtrip() {
        let data = create_test_data();
        let header = ElfHeader::new(&data).unwrap();
        let bytes = header.to_bytes();
        
        assert_eq!(data, bytes);
    }

    #[test]
    fn test_invalid_class() {
        let mut data = create_test_data();
        data[EI_CLASS] = 2; // Set to 64-bit
        assert!(matches!(ElfHeader::new(&data), Err(ElfHeaderError::InvalidClass)));
    }

    #[test]
    fn test_invalid_type() {
        let mut data = create_test_data();
        let fmt = ElfFormat::new(true);
        data[EI_NIDENT..EI_NIDENT + 2].copy_from_slice(&fmt.pack_u16(2)); // Not relocatable
        assert!(matches!(ElfHeader::new(&data), Err(ElfHeaderError::InvalidType)));
    }
}
