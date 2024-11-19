from typing import  Optional
from .header import ElfHeader
from .section import Section
from ..utils.constants import SHT_SYMTAB, SHT_MIPS_DEBUG, SHT_MIPS_GPTAB, SHT_NOBITS, SHT_NULL

class ElfFile:
    def __init__(self, data: bytes) -> None:
        self.data = data
        assert data[:4] == b'\x7fELF', "not an ELF file"

        self.elf_header = ElfHeader(data[0:52])
        self.fmt = self.elf_header.fmt

        offset, size = self.elf_header.e_shoff, self.elf_header.e_shentsize
        null_section = Section(self.fmt, data[offset:offset + size], data, 0)
        num_sections = self.elf_header.e_shnum or null_section.sh_size

        self.sections = [null_section]
        for i in range(1, num_sections):
            ind = offset + i * size
            self.sections.append(Section(self.fmt, data[ind:ind + size], data, i))

        symtab: Optional[Section] = None
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

    def find_section(self, name: str) -> Optional[Section]:
        for s in self.sections:
            if s.name == name:
                return s
        return None

    def add_section(self, name: str, sh_type: int, sh_flags: int, sh_link: int, sh_info: int, sh_addralign: int, sh_entsize: int, data: bytes) -> Section:
        shstr = self.sections[self.elf_header.e_shstrndx]
        sh_name = shstr.add_str(name)
        s = Section.from_parts(self.fmt, sh_name=sh_name, sh_type=sh_type,
                sh_flags=sh_flags, sh_link=sh_link, sh_info=sh_info,
                sh_addralign=sh_addralign, sh_entsize=sh_entsize, data=data,
                index=len(self.sections))
        self.sections.append(s)
        s.name = name
        s.late_init(self.sections)
        return s

    def drop_mdebug_gptab(self) -> None:
        # We can only drop sections at the end, since otherwise section
        # references might be wrong. Luckily, these sections typically are.
        while self.sections[-1].sh_type in [SHT_MIPS_DEBUG, SHT_MIPS_GPTAB]:
            self.sections.pop()

    def write(self, filename: str) -> None:
        outfile = open(filename, 'wb')
        outidx = 0
        def write_out(data: bytes) -> None:
            nonlocal outidx
            outfile.write(data)
            outidx += len(data)
        def pad_out(align: int) -> None:
            if align and outidx % align:
                write_out(b'\0' * (align - outidx % align))

        self.elf_header.e_shnum = len(self.sections)
        write_out(self.elf_header.to_bin())

        for s in self.sections:
            if s.sh_type != SHT_NOBITS and s.sh_type != SHT_NULL:
                pad_out(s.sh_addralign)
                old_offset = s.sh_offset
                s.sh_offset = outidx
                if s.sh_type == SHT_MIPS_DEBUG and s.sh_offset != old_offset:
                    # The .mdebug section has moved, relocate offsets
                    s.relocate_mdebug(old_offset)
                write_out(s.data)

        pad_out(4)
        self.elf_header.e_shoff = outidx
        for s in self.sections:
            write_out(s.header_to_bin())

        outfile.seek(0)
        outfile.write(self.elf_header.to_bin())
        outfile.close()
