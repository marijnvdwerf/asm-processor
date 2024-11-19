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
        if data.len() < 52 {
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

    pub fn to_bytes(&self, fmt: &ElfFormat) -> Result<Vec<u8>, Error> {
        let mut result = vec![0; 52];
        result[0..EI_NIDENT].copy_from_slice(&self.e_ident);
        fmt.pack_u16(&mut result[16..18], self.e_type)?;
        fmt.pack_u16(&mut result[18..20], self.e_machine)?;
        fmt.pack_u32(&mut result[20..24], self.e_version)?;
        fmt.pack_u32(&mut result[24..28], self.e_entry)?;
        fmt.pack_u32(&mut result[28..32], self.e_phoff)?;
        fmt.pack_u32(&mut result[32..36], self.e_shoff)?;
        fmt.pack_u32(&mut result[36..40], self.e_flags)?;
        fmt.pack_u16(&mut result[40..42], self.e_ehsize)?;
        fmt.pack_u16(&mut result[42..44], self.e_phentsize)?;
        fmt.pack_u16(&mut result[44..46], self.e_phnum)?;
        fmt.pack_u16(&mut result[46..48], self.e_shentsize)?;
        fmt.pack_u16(&mut result[48..50], self.e_shnum)?;
        fmt.pack_u16(&mut result[50..52], self.e_shstrndx)?;
        Ok(result)
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
        fmt.pack_u16(&mut data[16..18], 1).unwrap();

        // Set machine to MIPS (8)
        fmt.pack_u16(&mut data[18..20], 8).unwrap();

        // Set version
        fmt.pack_u32(&mut data[20..24], 1).unwrap();

        // Set shoff to non-zero
        fmt.pack_u32(&mut data[32..36], 1).unwrap();

        // Set shstrndx to non-zero
        fmt.pack_u16(&mut data[50..52], 1).unwrap();

        let header = ElfHeader::new(&fmt, &data).unwrap();
        assert_eq!(header.e_type, 1);
        assert_eq!(header.e_machine, 8);
        assert_eq!(header.e_shoff, 1);
        assert_eq!(header.e_shstrndx, 1);

        // Test round-trip
        let bytes = header.to_bytes(&fmt).unwrap();
        let header2 = ElfHeader::new(&fmt, &bytes).unwrap();
        assert_eq!(header.e_type, header2.e_type);
        assert_eq!(header.e_machine, header2.e_machine);
        assert_eq!(header.e_shoff, header2.e_shoff);
        assert_eq!(header.e_shstrndx, header2.e_shstrndx);
    }
}
