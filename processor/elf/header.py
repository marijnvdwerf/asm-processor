from .format import ElfFormat
from ..utils.constants import EI_NIDENT, EI_DATA,EI_CLASS, SHN_UNDEF

class ElfHeader:
    """
    typedef struct {
        unsigned char   e_ident[EI_NIDENT];
        Elf32_Half      e_type;
        Elf32_Half      e_machine;
        Elf32_Word      e_version;
        Elf32_Addr      e_entry;
        Elf32_Off       e_phoff;
        Elf32_Off       e_shoff;
        Elf32_Word      e_flags;
        Elf32_Half      e_ehsize;
        Elf32_Half      e_phentsize;
        Elf32_Half      e_phnum;
        Elf32_Half      e_shentsize;
        Elf32_Half      e_shnum;
        Elf32_Half      e_shstrndx;
    } Elf32_Ehdr;
    """

    def __init__(self, data: bytes) -> None:
        self.e_ident = data[:EI_NIDENT]
        assert self.e_ident[EI_CLASS] == 1 # 32-bit
        self.fmt = ElfFormat(is_big_endian=(self.e_ident[EI_DATA] == 2))
        self.e_type, self.e_machine, self.e_version, self.e_entry, self.e_phoff, self.e_shoff, self.e_flags, self.e_ehsize, self.e_phentsize, self.e_phnum, self.e_shentsize, self.e_shnum, self.e_shstrndx = self.fmt.unpack('HHIIIIIHHHHHH', data[EI_NIDENT:])
        assert self.e_type == 1 # relocatable
        assert self.e_machine == 8 # MIPS I Architecture
        assert self.e_phoff == 0 # no program header
        assert self.e_shoff != 0 # section header
        assert self.e_shstrndx != SHN_UNDEF

    def to_bin(self) -> bytes:
        return self.e_ident + self.fmt.pack('HHIIIIIHHHHHH', self.e_type,
                self.e_machine, self.e_version, self.e_entry, self.e_phoff,
                self.e_shoff, self.e_flags, self.e_ehsize, self.e_phentsize,
                self.e_phnum, self.e_shentsize, self.e_shnum, self.e_shstrndx)

