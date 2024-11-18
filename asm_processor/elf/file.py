from . import header
from . import section
from ..utils.constants import *

class ElfFile:
    def __init__(self, data):
        self.data = data
        assert data[:4] == b'\x7fELF', "not an ELF file"

        self.elf_header = header.ElfHeader(data[0:52])
        self.fmt = self.elf_header.fmt

        offset, size = self.elf_header.e_shoff, self.elf_header.e_shentsize
        null_section = section.Section(self.fmt, data[offset:offset + size], data, 0)
        num_sections = self.elf_header.e_shnum or null_section.sh_size

        self.sections = [null_section]
        for i in range(1, num_sections):
            ind = offset + i * size
            self.sections.append(section.Section(self.fmt, data[ind:ind + size], data, i))

        symtab = None
        for s in self.sections:
            if s.sh_type == SHT_SYMTAB:
                assert not symtab
                symtab = s
        assert symtab is not None
        self.symtab = symtab

        shstr = self.sections[self.elf_header.e_shstrndx]
        for s in self.sections:
            s.name = shstr.lookup_str(s.sh_name)
            s.late_init(self.sections)

    def find_section(self, name):
        for s in self.sections:
            if s.name == name:
                return s
        return None

    def add_section(self, name, sh_type, sh_flags, sh_link, sh_info, sh_addralign, sh_entsize, data):
        shstr = self.sections[self.elf_header.e_shstrndx]
        sh_name = shstr.add_str(name)
        s = section.Section.from_parts(self.fmt, sh_name, sh_type, sh_flags, sh_link, sh_info, sh_addralign, sh_entsize, data, len(self.sections))
        s.name = name
        self.sections.append(s)
        return s

    def drop_mdebug_gptab(self):
        # Drop unnecessary sections to minimize file size and avoid processing .mdebug
        # relocations. Also drop .gptab, which is like .rela.mdebug but for static variables.
        # These sections don't seem to serve any purpose in practice.
        filtered_sections = []
        for s in self.sections:
            if s.sh_type == SHT_MIPS_DEBUG:
                continue
            if s.name and '.gptab.' in s.name:
                continue
            filtered_sections.append(s)
        self.sections = filtered_sections

        # Fix up section indices
        index_map = {}
        for i, s in enumerate(self.sections):
            index_map[s.index] = i
            s.index = i
        self.elf_header.e_shnum = len(self.sections)

        # Fix up relocation indices (we don't need to fix up symbol indices since we
        # didn't drop any symbol tables)
        for s in self.sections:
            if s.sh_link in index_map:
                s.sh_link = index_map[s.sh_link]
            if s.sh_type in [SHT_REL, SHT_RELA]:
                s.sh_info = index_map[s.sh_info]

    def write(self, filename):
        outfile = open(filename, 'wb')
        outfile.write(self.data[:52])

        def write_out(data):
            outfile.write(data)
            return len(data)

        def pad_out(align):
            if align and (outfile.tell() % align):
                outfile.write(b'\0' * (align - outfile.tell() % align))

        # Put section data first
        for s in self.sections:
            if s.sh_type == SHT_NOBITS:
                continue
            if s.sh_type == SHT_NULL:
                continue
            pad_out(s.sh_addralign)
            s.sh_offset = outfile.tell()
            write_out(s.data)

        # Then section headers
        pad_out(4)
        self.elf_header.e_shoff = outfile.tell()
        for s in self.sections:
            write_out(s.header_to_bin())

        outfile.close()
