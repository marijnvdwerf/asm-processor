from typing import Optional
from ..utils.constants import SHN_XINDEX
from .format import ElfFormat
from .section import Section

class Symbol:
    """
    typedef struct {
        Elf32_Word      st_name;
        Elf32_Addr      st_value;
        Elf32_Word      st_size;
        unsigned char   st_info;
        unsigned char   st_other;
        Elf32_Half      st_shndx;
    } Elf32_Sym;
    """

    def __init__(self, fmt: ElfFormat, data: bytes, strtab: Section, name: Optional[str]=None) -> None:
        self.fmt = fmt
        self.st_name, self.st_value, self.st_size, st_info, self.st_other, self.st_shndx = fmt.unpack('IIIBBH', data)
        assert self.st_shndx != SHN_XINDEX, "too many sections (SHN_XINDEX not supported)"
        self.bind = st_info >> 4
        self.type = st_info & 15
        self.name = name if name is not None else strtab.lookup_str(self.st_name)
        self.visibility = self.st_other & 3

    @staticmethod
    def from_parts(fmt: ElfFormat, st_name: int, st_value: int, st_size: int, st_info: int, st_other: int, st_shndx: int, strtab: Section, name: str) -> "Symbol":
        header = fmt.pack('IIIBBH', st_name, st_value, st_size, st_info, st_other, st_shndx)
        return Symbol(fmt, header, strtab, name)

    def to_bin(self) -> bytes:
        st_info = (self.bind << 4) | self.type
        return self.fmt.pack('IIIBBH', self.st_name, self.st_value, self.st_size, st_info, self.st_other, self.st_shndx)
