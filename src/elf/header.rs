use crate::utils::Error;
use crate::elf::format::ElfFormat;
use crate::elf::constants::{EI_NIDENT, SHN_UNDEF};

#[derive(Debug)]
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

impl ElfHeader {
    pub fn new(fmt: &ElfFormat, data: &[u8]) -> Result<Self, Error> {
        if data.len() < EI_NIDENT {
            return Err(Error::InvalidFormat("Header too short".into()));
        }

        let mut e_ident = [0u8; EI_NIDENT];
        e_ident.copy_from_slice(&data[0..EI_NIDENT]);

        if e_ident[0] != 0x7F || e_ident[1] != b'E' || e_ident[2] != b'L' || e_ident[3] != b'F' {
            return Err(Error::InvalidFormat("Invalid ELF magic".into()));
        }

        let e_type = fmt.unpack_u16(&data[16..18])?;
        let e_machine = fmt.unpack_u16(&data[18..20])?;
        let e_version = fmt.unpack_u32(&data[20..24])?;
        let e_entry = fmt.unpack_u32(&data[24..28])?;
        let e_phoff = fmt.unpack_u32(&data[28..32])?;
        let e_shoff = fmt.unpack_u32(&data[32..36])?;
        let e_flags = fmt.unpack_u32(&data[36..40])?;
        let e_ehsize = fmt.unpack_u16(&data[40..42])?;
        let e_phentsize = fmt.unpack_u16(&data[42..44])?;
        let e_phnum = fmt.unpack_u16(&data[44..46])?;
        let e_shentsize = fmt.unpack_u16(&data[46..48])?;
        let e_shnum = fmt.unpack_u16(&data[48..50])?;
        let e_shstrndx = fmt.unpack_u16(&data[50..52])?;

        if e_type != 1 {
            return Err(Error::InvalidFormat("Not a relocatable file".into()));
        }

        if e_machine != 8 {
            return Err(Error::InvalidFormat("Not a MIPS file".into()));
        }

        if e_phoff != 0 {
            return Err(Error::InvalidFormat("Unexpected program header table".into()));
        }

        if e_shoff == 0 {
            return Err(Error::InvalidFormat("No section header table".into()));
        }

        if e_shstrndx == SHN_UNDEF {
            return Err(Error::InvalidFormat("No section name string table".into()));
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_parse() {
        let fmt = ElfFormat::new(true);
        let mut data = vec![0; 52];

        // Set magic bytes
        data[0] = 0x7F;
        data[1] = b'E';
        data[2] = b'L';
        data[3] = b'F';

        // Set type to ET_REL (1)
        data[16] = 0;
        data[17] = 1;

        // Set machine to MIPS (8)
        data[18] = 0;
        data[19] = 8;

        // Set version
        data[20] = 0;
        data[21] = 0;
        data[22] = 0;
        data[23] = 1;

        // Set shoff to non-zero
        data[32] = 0;
        data[33] = 0;
        data[34] = 0;
        data[35] = 1;

        // Set shstrndx to non-zero
        data[50] = 0;
        data[51] = 1;

        let header = ElfHeader::new(&fmt, &data).unwrap();
        assert_eq!(header.e_type, 1);
        assert_eq!(header.e_machine, 8);
        assert_eq!(header.e_shoff, 1);
        assert_eq!(header.e_shstrndx, 1);
    }
}
