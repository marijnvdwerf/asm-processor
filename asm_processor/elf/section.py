from ..utils.constants import *
from . import symbol
from . import relocation

class Section:
    """typedef struct {
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
    } Elf32_Shdr;"""
    def __init__(self, fmt, header, data, index):
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
        self.relocated_by = []

    @staticmethod
    def from_parts(fmt, sh_name, sh_type, sh_flags, sh_link, sh_info, sh_addralign, sh_entsize, data, index):
        header = fmt.pack('IIIIIIIIII', sh_name, sh_type, sh_flags, 0, 0, len(data), sh_link, sh_info, sh_addralign, sh_entsize)
        return Section(fmt, header, data, index)

    def lookup_str(self, index):
        assert self.sh_type == SHT_STRTAB
        to = self.data.find(b'\0', index)
        assert to != -1
        return self.data[index:to].decode('latin1')

    def add_str(self, string):
        assert self.sh_type == SHT_STRTAB
        ret = len(self.data)
        self.data += string.encode('latin1') + b'\0'
        return ret

    def is_rel(self):
        return self.sh_type == SHT_REL or self.sh_type == SHT_RELA

    def header_to_bin(self):
        if self.sh_type != SHT_NOBITS:
            self.sh_size = len(self.data)
        return self.fmt.pack('IIIIIIIIII',
            self.sh_name, self.sh_type, self.sh_flags, self.sh_addr,
            self.sh_offset, self.sh_size, self.sh_link, self.sh_info,
            self.sh_addralign, self.sh_entsize)

    def late_init(self, sections):
        if self.sh_type == SHT_SYMTAB:
            self.init_symbols(sections)
        elif self.is_rel():
            self.init_relocs()

    def find_symbol(self, name):
        assert self.sh_type == SHT_SYMTAB
        for s in self.symbol_entries:
            if s.name == name:
                return s
        return None

    def find_symbol_in_section(self, name, section):
        assert self.sh_type == SHT_SYMTAB
        for s in self.symbol_entries:
            if s.st_shndx == section.index and s.name == name:
                return s
        return None

    def init_symbols(self, sections):
        assert self.sh_type == SHT_SYMTAB
        assert self.sh_entsize == 16
        self.strtab = sections[self.sh_link]
        entries = []
        for pos in range(0, self.sh_size, self.sh_entsize):
            entries.append(symbol.Symbol(self.fmt, self.data[pos:pos+self.sh_entsize], self.strtab))
        self.symbol_entries = entries

    def init_relocs(self):
        assert self.sh_entsize in [8, 12]
        entries = []
        for pos in range(0, self.sh_size, self.sh_entsize):
            entries.append(relocation.Relocation(self.fmt, self.data[pos:pos+self.sh_entsize], self.sh_type))
        self.relocations = entries

    def local_symbols(self):
        assert self.sh_type == SHT_SYMTAB
        return self.symbol_entries[:self.sh_info]

    def global_symbols(self):
        assert self.sh_type == SHT_SYMTAB
        return self.symbol_entries[self.sh_info:]

    def relocate_mdebug(self, original_offset):
        assert self.sh_type == SHT_MIPS_DEBUG
        new_data = bytearray()
        pos = 0
        # Skip the header for now
        pos += 6 * 4
        new_data += self.data[0:pos]
        while pos < len(self.data):
            # Read the next "block"
            descriptor = self.fmt.unpack('H', self.data[pos:pos+2])[0]
            pos += 2
            new_data += self.fmt.pack('H', descriptor)
            # The symbol type dictates what data follows
            if descriptor == MIPS_DEBUG_ST_STATIC:
                val = self.fmt.unpack('III', self.data[pos:pos+12])
                pos += 12
                if val[1] != 0:
                    val = (val[0], val[1] + original_offset, val[2])
                new_data += self.fmt.pack('III', *val)
            elif descriptor == MIPS_DEBUG_ST_STATIC_PROC:
                val = self.fmt.unpack('IIII', self.data[pos:pos+16])
                pos += 16
                if val[1] != 0:
                    val = (val[0], val[1] + original_offset, val[2], val[3])
                new_data += self.fmt.pack('IIII', *val)
            elif descriptor == MIPS_DEBUG_ST_END:
                pass
            elif descriptor == MIPS_DEBUG_ST_PROC:
                val = self.fmt.unpack('IIII', self.data[pos:pos+16])
                pos += 16
                if val[1] != 0:
                    val = (val[0], val[1] + original_offset, val[2], val[3])
                new_data += self.fmt.pack('IIII', *val)
            elif descriptor == MIPS_DEBUG_ST_FILE:
                strlen = self.fmt.unpack('I', self.data[pos:pos+4])[0]
                pos += 4
                new_data += self.fmt.pack('I', strlen)
                new_data += self.data[pos:pos+strlen]
                pos += strlen
            elif descriptor == MIPS_DEBUG_ST_BLOCK:
                val = self.fmt.unpack('II', self.data[pos:pos+8])
                pos += 8
                if val[0] != 0:
                    val = (val[0] + original_offset, val[1] + original_offset)
                new_data += self.fmt.pack('II', *val)
            elif descriptor in [MIPS_DEBUG_ST_STRUCT, MIPS_DEBUG_ST_UNION, MIPS_DEBUG_ST_ENUM]:
                strlen = self.fmt.unpack('I', self.data[pos:pos+4])[0]
                pos += 4
                new_data += self.fmt.pack('I', strlen)
                new_data += self.data[pos:pos+strlen]
                pos += strlen
                # Read the tag list
                while True:
                    tag = self.fmt.unpack('I', self.data[pos:pos+4])[0]
                    pos += 4
                    new_data += self.fmt.pack('I', tag)
                    if tag == 0:
                        break
            else:
                print("Unknown mdebug symbol type:", descriptor)
                return None
        self.data = new_data
