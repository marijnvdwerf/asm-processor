from typing import List, Optional,  Tuple
from .format import ElfFormat
from .symbol import Symbol
from .relocation import Relocation
from ..utils.constants import SHT_SYMTAB, SHT_REL, SHT_RELA, SHT_NOBITS, SHT_STRTAB, SHT_MIPS_DEBUG, SHF_LINK_ORDER

class Section:
    """
    typedef struct {
        Elf32_Word   sh_name;
        Elf32_Word   sh_type;
        Elf32_Word   sh_flags;
        Elf32_Addr   sh_addr;
        Elf32_Off    sh_offset;
        Elf32_Word   sh_size;
        Elf32_Word   sh_link;
        Elf32_Word   sh_info;
        Elf32_Word   sh_addralign;
        Elf32_Word   sh_entsize;
    } Elf32_Shdr;
    """

    def __init__(self, fmt: ElfFormat, header: bytes, data: bytes, index: int) -> None:
        self.fmt = fmt
        self.sh_name, self.sh_type, self.sh_flags, self.sh_addr, self.sh_offset, self.sh_size, self.sh_link, self.sh_info, self.sh_addralign, self.sh_entsize = fmt.unpack('IIIIIIIIII', header)
        assert not self.sh_flags & SHF_LINK_ORDER
        if self.sh_entsize != 0:
            assert self.sh_size % self.sh_entsize == 0
        if self.sh_type == SHT_NOBITS:
            self.data = b''
        else:
            self.data = data[self.sh_offset:self.sh_offset + self.sh_size]
        self.index = index
        self.relocated_by: List[Section] = []
        self.name = ""

    @staticmethod
    def from_parts(fmt: ElfFormat, sh_name: int, sh_type: int, sh_flags: int, sh_link: int, sh_info: int, sh_addralign: int, sh_entsize: int, data: bytes, index: int) -> "Section":
        header = fmt.pack('IIIIIIIIII', sh_name, sh_type, sh_flags, 0, 0, len(data), sh_link, sh_info, sh_addralign, sh_entsize)
        return Section(fmt, header, data, index)

    def lookup_str(self, index: int) -> str:
        assert self.sh_type == SHT_STRTAB
        to = self.data.find(b'\0', index)
        assert to != -1
        return self.data[index:to].decode('latin1')

    def add_str(self, string: str) -> int:
        assert self.sh_type == SHT_STRTAB
        ret = len(self.data)
        self.data += string.encode('latin1') + b'\0'
        return ret

    def is_rel(self) -> bool:
        return self.sh_type == SHT_REL or self.sh_type == SHT_RELA

    def header_to_bin(self) -> bytes:
        if self.sh_type != SHT_NOBITS:
            self.sh_size = len(self.data)
        return self.fmt.pack('IIIIIIIIII', self.sh_name, self.sh_type, self.sh_flags, self.sh_addr, self.sh_offset, self.sh_size, self.sh_link, self.sh_info, self.sh_addralign, self.sh_entsize)

    def late_init(self, sections: List["Section"]) -> None:
        if self.sh_type == SHT_SYMTAB:
            self.init_symbols(sections)
        elif self.is_rel():
            self.rel_target = sections[self.sh_info]
            self.rel_target.relocated_by.append(self)
            self.init_relocs()

    def find_symbol(self, name: str) -> Optional[Tuple[int, int]]:
        assert self.sh_type == SHT_SYMTAB
        for s in self.symbol_entries:
            if s.name == name:
                return (s.st_shndx, s.st_value)
        return None

    def find_symbol_in_section(self, name: str, section: "Section") -> int:
        pos = self.find_symbol(name)
        assert pos is not None
        assert pos[0] == section.index
        return pos[1]

    def init_symbols(self, sections: List["Section"]) -> None:
        assert self.sh_type == SHT_SYMTAB
        assert self.sh_entsize == 16
        self.strtab = sections[self.sh_link]
        entries = []
        for i in range(0, self.sh_size, self.sh_entsize):
            entries.append(Symbol(self.fmt, self.data[i:i+self.sh_entsize], self.strtab))
        self.symbol_entries = entries

    def init_relocs(self) -> None:
        assert self.is_rel()
        entries = []
        for i in range(0, self.sh_size, self.sh_entsize):
            entries.append(Relocation(self.fmt, self.data[i:i+self.sh_entsize], self.sh_type))
        self.relocations = entries

    def local_symbols(self) -> List[Symbol]:
        assert self.sh_type == SHT_SYMTAB
        return self.symbol_entries[:self.sh_info]

    def global_symbols(self) -> List[Symbol]:
        assert self.sh_type == SHT_SYMTAB
        return self.symbol_entries[self.sh_info:]

    def relocate_mdebug(self, original_offset: int) -> None:
        assert self.sh_type == SHT_MIPS_DEBUG
        new_data = bytearray(self.data)
        shift_by = self.sh_offset - original_offset

        # Update the file-relative offsets in the Symbolic HDRR
        hdrr_magic, hdrr_vstamp, hdrr_ilineMax, hdrr_cbLine, \
            hdrr_cbLineOffset, hdrr_idnMax, hdrr_cbDnOffset, hdrr_ipdMax, \
            hdrr_cbPdOffset, hdrr_isymMax, hdrr_cbSymOffset, hdrr_ioptMax, \
            hdrr_cbOptOffset, hdrr_iauxMax, hdrr_cbAuxOffset, hdrr_issMax, \
            hdrr_cbSsOffset, hdrr_issExtMax, hdrr_cbSsExtOffset, hdrr_ifdMax, \
            hdrr_cbFdOffset, hdrr_crfd, hdrr_cbRfdOffset, hdrr_iextMax, \
            hdrr_cbExtOffset = self.fmt.unpack("HHIIIIIIIIIIIIIIIIIIIIIII", self.data[0:0x60])

        assert hdrr_magic == 0x7009, "Invalid magic value for .mdebug symbolic header"

        if hdrr_cbLine: hdrr_cbLineOffset += shift_by
        if hdrr_idnMax: hdrr_cbDnOffset += shift_by
        if hdrr_ipdMax: hdrr_cbPdOffset += shift_by
        if hdrr_isymMax: hdrr_cbSymOffset += shift_by
        if hdrr_ioptMax: hdrr_cbOptOffset += shift_by
        if hdrr_iauxMax: hdrr_cbAuxOffset += shift_by
        if hdrr_issMax: hdrr_cbSsOffset += shift_by
        if hdrr_issExtMax: hdrr_cbSsExtOffset += shift_by
        if hdrr_ifdMax: hdrr_cbFdOffset += shift_by
        if hdrr_crfd: hdrr_cbRfdOffset += shift_by
        if hdrr_iextMax: hdrr_cbExtOffset += shift_by

        new_data[0:0x60] = self.fmt.pack("HHIIIIIIIIIIIIIIIIIIIIIII", hdrr_magic, hdrr_vstamp, hdrr_ilineMax, hdrr_cbLine, \
            hdrr_cbLineOffset, hdrr_idnMax, hdrr_cbDnOffset, hdrr_ipdMax, \
            hdrr_cbPdOffset, hdrr_isymMax, hdrr_cbSymOffset, hdrr_ioptMax, \
            hdrr_cbOptOffset, hdrr_iauxMax, hdrr_cbAuxOffset, hdrr_issMax, \
            hdrr_cbSsOffset, hdrr_issExtMax, hdrr_cbSsExtOffset, hdrr_ifdMax, \
            hdrr_cbFdOffset, hdrr_crfd, hdrr_cbRfdOffset, hdrr_iextMax, \
            hdrr_cbExtOffset)

        self.data = bytes(new_data)
