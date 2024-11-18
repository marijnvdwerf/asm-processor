#!/usr/bin/env python3
import argparse
import tempfile
import struct
import sys
import re
import os
from io import StringIO

from processor.elf.symbol import Symbol
from processor.elf.file import ElfFile

from processor.utils.state import GlobalState
from processor.utils.errors import Failure
from processor.asm import GlobalAsmBlock

from processor.utils.options import Opts

MAX_FN_SIZE = 100
SLOW_CHECKS = False

EI_NIDENT     = 16
EI_CLASS      = 4
EI_DATA       = 5
EI_VERSION    = 6
EI_OSABI      = 7
EI_ABIVERSION = 8
STN_UNDEF = 0

SHN_UNDEF     = 0
SHN_ABS       = 0xfff1
SHN_COMMON    = 0xfff2
SHN_XINDEX    = 0xffff
SHN_LORESERVE = 0xff00

STT_NOTYPE  = 0
STT_OBJECT  = 1
STT_FUNC    = 2
STT_SECTION = 3
STT_FILE    = 4
STT_COMMON  = 5
STT_TLS     = 6

STB_LOCAL  = 0
STB_GLOBAL = 1
STB_WEAK   = 2

STV_DEFAULT   = 0
STV_INTERNAL  = 1
STV_HIDDEN    = 2
STV_PROTECTED = 3

SHT_NULL          = 0
SHT_PROGBITS      = 1
SHT_SYMTAB        = 2
SHT_STRTAB        = 3
SHT_RELA          = 4
SHT_HASH          = 5
SHT_DYNAMIC       = 6
SHT_NOTE          = 7
SHT_NOBITS        = 8
SHT_REL           = 9
SHT_SHLIB         = 10
SHT_DYNSYM        = 11
SHT_INIT_ARRAY    = 14
SHT_FINI_ARRAY    = 15
SHT_PREINIT_ARRAY = 16
SHT_GROUP         = 17
SHT_SYMTAB_SHNDX  = 18
SHT_MIPS_GPTAB    = 0x70000003
SHT_MIPS_DEBUG    = 0x70000005
SHT_MIPS_REGINFO  = 0x70000006
SHT_MIPS_OPTIONS  = 0x7000000d

SHF_WRITE            = 0x1
SHF_ALLOC            = 0x2
SHF_EXECINSTR        = 0x4
SHF_MERGE            = 0x10
SHF_STRINGS          = 0x20
SHF_INFO_LINK        = 0x40
SHF_LINK_ORDER       = 0x80
SHF_OS_NONCONFORMING = 0x100
SHF_GROUP            = 0x200
SHF_TLS              = 0x400

R_MIPS_32   = 2
R_MIPS_26   = 4
R_MIPS_HI16 = 5
R_MIPS_LO16 = 6

MIPS_DEBUG_ST_STATIC = 2
MIPS_DEBUG_ST_PROC = 6
MIPS_DEBUG_ST_BLOCK = 7
MIPS_DEBUG_ST_END = 8
MIPS_DEBUG_ST_FILE = 11
MIPS_DEBUG_ST_STATIC_PROC = 14
MIPS_DEBUG_ST_STRUCT = 26
MIPS_DEBUG_ST_UNION = 27
MIPS_DEBUG_ST_ENUM = 28


def is_temp_name(name):
    return name.startswith('_asmpp_')





cutscene_data_regexpr = re.compile(r"CutsceneData (.|\n)*\[\] = {")
float_regexpr = re.compile(r"[-+]?[0-9]*\.?[0-9]+([eE][-+]?[0-9]+)?f")

def repl_float_hex(m):
    return str(struct.unpack(">I", struct.pack(">f", float(m.group(0).strip().rstrip("f"))))[0])

def parse_source(f, opts, out_dependencies, print_source=None):
    if opts.opt in ['O1', 'O2']:
        if opts.framepointer:
            min_instr_count = 6
            skip_instr_count = 5
        else:
            min_instr_count = 2
            skip_instr_count = 1
    elif opts.opt == 'O0':
        if opts.framepointer:
            min_instr_count = 8
            skip_instr_count = 8
        else:
            min_instr_count = 4
            skip_instr_count = 4
    elif opts.opt == 'g':
        if opts.framepointer:
            min_instr_count = 7
            skip_instr_count = 7
        else:
            min_instr_count = 4
            skip_instr_count = 4
    elif opts.opt == 'g3':
        if opts.framepointer:
            min_instr_count = 4
            skip_instr_count = 4
        else:
            min_instr_count = 2
            skip_instr_count = 2
    else:
        raise Failure("must pass one of -g, -O0, -O1, -O2, -O2 -g3")
    prelude_if_late_rodata = 0
    if opts.kpic:
        # Without optimizations, the PIC prelude always takes up 3 instructions.
        # With optimizations, the prelude is optimized out if there's no late rodata.
        if opts.opt in ('g3', 'O2'):
            prelude_if_late_rodata = 3
        else:
            min_instr_count += 3
            skip_instr_count += 3

    use_jtbl_for_rodata = False
    if opts.opt in ['O2', 'g3'] and not opts.framepointer and not opts.kpic:
        use_jtbl_for_rodata = True

    state = GlobalState(min_instr_count, skip_instr_count, use_jtbl_for_rodata, prelude_if_late_rodata, opts.mips1, opts.pascal)
    output_enc = opts.output_enc

    global_asm = None
    asm_functions = []
    output_lines = [
        '#line 1 "' + f.name + '"'
    ]

    is_cutscene_data = False
    is_early_include = False

    for line_no, raw_line in enumerate(f, 1):
        raw_line = raw_line.rstrip()
        line = raw_line.lstrip()

        # Print exactly one output line per source line, to make compiler
        # errors have correct line numbers. These will be overridden with
        # reasonable content further down.
        output_lines.append('')

        if global_asm is not None:
            if line.startswith(')'):
                src, fn = global_asm.finish(state)
                for i, line2 in enumerate(src):
                    output_lines[start_index + i] = line2
                asm_functions.append(fn)
                global_asm = None
            else:
                global_asm.process_line(raw_line, output_enc)
        elif line in ("GLOBAL_ASM(", "#pragma GLOBAL_ASM("):
            global_asm = GlobalAsmBlock("GLOBAL_ASM block at line " + str(line_no))
            start_index = len(output_lines)
        elif (
            (line.startswith('GLOBAL_ASM("') or line.startswith('#pragma GLOBAL_ASM("'))
            and line.endswith('")')
        ) or (
            (line.startswith('INCLUDE_ASM("') or line.startswith('INCLUDE_RODATA("'))
            and '",' in line
            and line.endswith(");")
        ):
            prologue = []
            if line.startswith("INCLUDE_"):
                # INCLUDE_ASM("path/to", functionname);
                before, after = line.split('",', 1)
                fname = before[before.index("(") + 2 :] + "/" + after.strip()[:-2] + ".s"
                if line.startswith("INCLUDE_RODATA"):
                    prologue = [".section .rodata"]
            else:
                # GLOBAL_ASM("path/to/file.s")
                fname = line[line.index("(") + 2 : -2]
            ext_global_asm = GlobalAsmBlock(fname)
            for line2 in prologue:
                ext_global_asm.process_line(line2, output_enc)
            try:
                f = open(fname, encoding=opts.input_enc)
            except FileNotFoundError:
                # The GLOBAL_ASM block might be surrounded by an ifdef, so it's
                # not clear whether a missing file actually represents a compile
                # error. Pass the responsibility for determining that on to the
                # compiler by emitting a bad include directive. (IDO treats
                # #error as a warning for some reason.)
                output_lines[-1] = f"#include \"GLOBAL_ASM:{fname}\""
                continue
            with f:
                for line2 in f:
                    ext_global_asm.process_line(line2.rstrip(), output_enc)
            src, fn = ext_global_asm.finish(state)
            output_lines[-1] = ''.join(src)
            asm_functions.append(fn)
            out_dependencies.append(fname)
        elif line == '#pragma asmproc recurse':
            # C includes qualified as
            # #pragma asmproc recurse
            # #include "file.c"
            # will be processed recursively when encountered
            is_early_include = True
        elif is_early_include:
            # Previous line was a #pragma asmproc recurse
            is_early_include = False
            if not line.startswith("#include "):
                raise Failure("#pragma asmproc recurse must be followed by an #include ")
            fpath = os.path.dirname(f.name)
            fname = os.path.join(fpath, line[line.index(' ') + 2 : -1])
            out_dependencies.append(fname)
            include_src = StringIO()
            with open(fname, encoding=opts.input_enc) as include_file:
                parse_source(include_file, opts, out_dependencies, include_src)
            include_src.write('#line ' + str(line_no + 1) + ' "' + f.name + '"')
            output_lines[-1] = include_src.getvalue()
            include_src.close()
        else:
            if opts.enable_cutscene_data_float_encoding:
                # This is a hack to replace all floating-point numbers in an array of a particular type
                # (in this case CutsceneData) with their corresponding IEEE-754 hexadecimal representation
                if cutscene_data_regexpr.search(line) is not None:
                    is_cutscene_data = True
                elif line.endswith("};"):
                    is_cutscene_data = False
                if is_cutscene_data:
                    raw_line = re.sub(float_regexpr, repl_float_hex, raw_line)
            output_lines[-1] = raw_line

    if print_source:
        if isinstance(print_source, StringIO):
            for line in output_lines:
                print_source.write(line + '\n')
        else:
            newline_encoded = "\n".encode(output_enc)
            for line in output_lines:
                try:
                    line_encoded = line.encode(output_enc)
                except UnicodeEncodeError:
                    print("Failed to encode a line to", output_enc)
                    print("The line:", line)
                    print("The line, utf-8-encoded:", line.encode("utf-8"))
                    raise
                print_source.write(line_encoded)
                print_source.write(newline_encoded)
            print_source.flush()

    return asm_functions

def fixup_objfile(objfile_name, functions, asm_prelude, assembler, output_enc, drop_mdebug_gptab, convert_statics):
    SECTIONS = ['.data', '.text', '.rodata', '.bss']

    with open(objfile_name, 'rb') as f:
        objfile = ElfFile(f.read())
    fmt = objfile.fmt

    prev_locs = {
        '.text': 0,
        '.data': 0,
        '.rodata': 0,
        '.bss': 0,
    }
    to_copy = {
        '.text': [],
        '.data': [],
        '.rodata': [],
        '.bss': [],
    }
    asm = []
    all_late_rodata_dummy_bytes = []
    all_jtbl_rodata_size = []
    late_rodata_asm = []
    late_rodata_source_name_start = None
    late_rodata_source_name_end = None

    # Generate an assembly file with all the assembly we need to fill in. For
    # simplicity we pad with nops/.space so that addresses match exactly, so we
    # don't have to fix up relocations/symbol references.
    all_text_glabels = set()
    func_sizes = {}
    for function in functions:
        ifdefed = False
        for sectype, (temp_name, size) in function.data.items():
            if temp_name is None:
                continue
            assert size > 0
            loc = objfile.symtab.find_symbol(temp_name)
            if loc is None:
                ifdefed = True
                break
            loc = loc[1]
            prev_loc = prev_locs[sectype]
            if loc < prev_loc:
                # If the dummy C generates too little asm, and we have two
                # consecutive GLOBAL_ASM blocks, we detect that error here.
                # On the other hand, if it generates too much, we don't have
                # a good way of discovering that error: it's indistinguishable
                # from a static symbol occurring after the GLOBAL_ASM block.
                raise Failure("Wrongly computed size for section {} (diff {}). This is an asm-processor bug!".format(sectype, prev_loc- loc))
            if loc != prev_loc:
                asm.append('.section ' + sectype)
                if sectype == '.text':
                    for i in range((loc - prev_loc) // 4):
                        asm.append('nop')
                else:
                    asm.append('.space {}'.format(loc - prev_loc))
            to_copy[sectype].append((loc, size, temp_name, function.fn_desc))
            if function.text_glabels and sectype == '.text':
                func_sizes[function.text_glabels[0]] = size
            prev_locs[sectype] = loc + size
        if not ifdefed:
            all_text_glabels.update(function.text_glabels)
            all_late_rodata_dummy_bytes.append(function.late_rodata_dummy_bytes)
            all_jtbl_rodata_size.append(function.jtbl_rodata_size)
            late_rodata_asm.append(function.late_rodata_asm_conts)
            for sectype, (temp_name, size) in function.data.items():
                if temp_name is not None:
                    asm.append('.section ' + sectype)
                    asm.append('glabel ' + temp_name + '_asm_start')
            asm.append('.text')
            for line in function.asm_conts:
                asm.append(line)
            for sectype, (temp_name, size) in function.data.items():
                if temp_name is not None:
                    asm.append('.section ' + sectype)
                    asm.append('glabel ' + temp_name + '_asm_end')
    if any(late_rodata_asm):
        late_rodata_source_name_start = '_asmpp_late_rodata_start'
        late_rodata_source_name_end = '_asmpp_late_rodata_end'
        asm.append('.section .late_rodata')
        # Put some padding at the start to avoid conflating symbols with
        # references to the whole section.
        asm.append('.word 0, 0')
        asm.append('glabel {}'.format(late_rodata_source_name_start))
        for conts in late_rodata_asm:
            asm.extend(conts)
        asm.append('glabel {}'.format(late_rodata_source_name_end))

    o_file = tempfile.NamedTemporaryFile(prefix='asm-processor', suffix='.o', delete=False)
    o_name = o_file.name
    o_file.close()
    s_file = tempfile.NamedTemporaryFile(prefix='asm-processor', suffix='.s', delete=False)
    s_name = s_file.name
    try:
        s_file.write(asm_prelude + b'\n')
        for line in asm:
            s_file.write(line.encode(output_enc) + b'\n')
        s_file.close()
        ret = os.system(assembler + " " + s_name + " -o " + o_name)
        if ret != 0:
            raise Failure("failed to assemble")
        with open(o_name, 'rb') as f:
            asm_objfile = ElfFile(f.read())

        # Remove clutter from objdump output for tests, and make the tests
        # portable by avoiding absolute paths. Outside of tests .mdebug is
        # useful for showing source together with asm, though.
        mdebug_section = objfile.find_section('.mdebug')
        if drop_mdebug_gptab:
            objfile.drop_mdebug_gptab()

        # Unify reginfo sections
        target_reginfo = objfile.find_section('.reginfo')
        if target_reginfo is not None:
            source_reginfo_data = list(asm_objfile.find_section('.reginfo').data)
            data = list(target_reginfo.data)
            for i in range(20):
                data[i] |= source_reginfo_data[i]
            target_reginfo.data = bytes(data)

        # Move over section contents
        modified_text_positions = set()
        jtbl_rodata_positions = set()
        last_rodata_pos = 0
        for sectype in SECTIONS:
            if not to_copy[sectype]:
                continue
            source = asm_objfile.find_section(sectype)
            assert source is not None, "didn't find source section: " + sectype
            for (pos, count, temp_name, fn_desc) in to_copy[sectype]:
                loc1 = asm_objfile.symtab.find_symbol_in_section(temp_name + '_asm_start', source)
                loc2 = asm_objfile.symtab.find_symbol_in_section(temp_name + '_asm_end', source)
                assert loc1 == pos, "assembly and C files don't line up for section " + sectype + ", " + fn_desc
                if loc2 - loc1 != count:
                    raise Failure("incorrectly computed size for section " + sectype + ", " + fn_desc + ". If using .double, make sure to provide explicit alignment padding.")
            if sectype == '.bss':
                continue
            target = objfile.find_section(sectype)
            assert target is not None, "missing target section of type " + sectype
            data = list(target.data)
            for (pos, count, _, _) in to_copy[sectype]:
                data[pos:pos + count] = source.data[pos:pos + count]
                if sectype == '.text':
                    assert count % 4 == 0
                    assert pos % 4 == 0
                    for i in range(count // 4):
                        modified_text_positions.add(pos + 4 * i)
                elif sectype == '.rodata':
                    last_rodata_pos = pos + count
            target.data = bytes(data)

        # Move over late rodata. This is heuristic, sadly, since I can't think
        # of another way of doing it.
        moved_late_rodata = {}
        if any(all_late_rodata_dummy_bytes) or any(all_jtbl_rodata_size):
            source = asm_objfile.find_section('.late_rodata')
            target = objfile.find_section('.rodata')
            source_pos = asm_objfile.symtab.find_symbol_in_section(late_rodata_source_name_start, source)
            source_end = asm_objfile.symtab.find_symbol_in_section(late_rodata_source_name_end, source)
            if source_end - source_pos != sum(map(len, all_late_rodata_dummy_bytes)) * 4 + sum(all_jtbl_rodata_size):
                raise Failure("computed wrong size of .late_rodata")
            new_data = list(target.data)
            for dummy_bytes_list, jtbl_rodata_size in zip(all_late_rodata_dummy_bytes, all_jtbl_rodata_size):
                for index, dummy_bytes in enumerate(dummy_bytes_list):
                    if not fmt.is_big_endian:
                        dummy_bytes = dummy_bytes[::-1]
                    pos = target.data.index(dummy_bytes, last_rodata_pos)
                    # This check is nice, but makes time complexity worse for large files:
                    if SLOW_CHECKS and target.data.find(dummy_bytes, pos + 4) != -1:
                        raise Failure("multiple occurrences of late_rodata hex magic. Change asm-processor to use something better than 0xE0123456!")
                    if index == 0 and len(dummy_bytes_list) > 1 and target.data[pos+4:pos+8] == b'\0\0\0\0':
                        # Ugly hack to handle double alignment for non-matching builds.
                        # We were told by .late_rodata_alignment (or deduced from a .double)
                        # that a function's late_rodata started out 4 (mod 8), and emitted
                        # a float and then a double. But it was actually 0 (mod 8), so our
                        # double was moved by 4 bytes. To make them adjacent to keep jump
                        # tables correct, move the float by 4 bytes as well.
                        new_data[pos:pos+4] = b'\0\0\0\0'
                        pos += 4
                    new_data[pos:pos+4] = source.data[source_pos:source_pos+4]
                    moved_late_rodata[source_pos] = pos
                    last_rodata_pos = pos + 4
                    source_pos += 4
                if jtbl_rodata_size > 0:
                    assert dummy_bytes_list, "should always have dummy bytes before jtbl data"
                    pos = last_rodata_pos
                    new_data[pos : pos + jtbl_rodata_size] = \
                        source.data[source_pos : source_pos + jtbl_rodata_size]
                    for i in range(0, jtbl_rodata_size, 4):
                        moved_late_rodata[source_pos + i] = pos + i
                        jtbl_rodata_positions.add(pos + i)
                    last_rodata_pos += jtbl_rodata_size
                    source_pos += jtbl_rodata_size
            target.data = bytes(new_data)

        # Merge strtab data.
        strtab_adj = len(objfile.symtab.strtab.data)
        objfile.symtab.strtab.data += asm_objfile.symtab.strtab.data

        # Find relocated symbols
        relocated_symbols = set()
        for sectype in SECTIONS + ['.late_rodata']:
            for obj in [asm_objfile, objfile]:
                sec = obj.find_section(sectype)
                if sec is None:
                    continue
                for reltab in sec.relocated_by:
                    for rel in reltab.relocations:
                        relocated_symbols.add(obj.symtab.symbol_entries[rel.sym_index])

        # Move over symbols, deleting the temporary function labels.
        # Skip over new local symbols that aren't relocated against, to
        # avoid conflicts.
        empty_symbol = objfile.symtab.symbol_entries[0]
        new_syms = [s for s in objfile.symtab.symbol_entries[1:] if not is_temp_name(s.name)]

        for i, s in enumerate(asm_objfile.symtab.symbol_entries):
            is_local = (i < asm_objfile.symtab.sh_info)
            if is_local and s not in relocated_symbols:
                continue
            if is_temp_name(s.name):
                assert s not in relocated_symbols
                continue
            if s.st_shndx not in [SHN_UNDEF, SHN_ABS]:
                section_name = asm_objfile.sections[s.st_shndx].name
                target_section_name = section_name
                if section_name == ".late_rodata":
                    target_section_name = ".rodata"
                elif section_name not in SECTIONS:
                    raise Failure("generated assembly .o must only have symbols for .text, .data, .rodata, .late_rodata, ABS and UNDEF, but found " + section_name)
                objfile_section = objfile.find_section(target_section_name)
                if objfile_section is None:
                    raise Failure("generated assembly .o has section that real objfile lacks: " + target_section_name)
                s.st_shndx = objfile_section.index
                # glabel's aren't marked as functions, making objdump output confusing. Fix that.
                if s.name in all_text_glabels:
                    s.type = STT_FUNC
                    if s.name in func_sizes:
                        s.st_size = func_sizes[s.name]
                if section_name == '.late_rodata':
                    if s.st_value == 0:
                        # This must be a symbol corresponding to the whole .late_rodata
                        # section, being referred to from a relocation.
                        # Moving local symbols is tricky, because it requires fixing up
                        # lo16/hi16 relocation references to .late_rodata+<offset>.
                        # Just disallow it for now.
                        raise Failure("local symbols in .late_rodata are not allowed")
                    s.st_value = moved_late_rodata[s.st_value]
            s.st_name += strtab_adj
            new_syms.append(s)
        make_statics_global = convert_statics in ("global", "global-with-filename")

        # Add static symbols from .mdebug, so they can be referred to from GLOBAL_ASM
        if mdebug_section and convert_statics != "no":
            static_name_count = {}
            strtab_index = len(objfile.symtab.strtab.data)
            new_strtab_data = []
            ifd_max, cb_fd_offset = fmt.unpack('II', mdebug_section.data[18*4 : 20*4])
            cb_sym_offset, = fmt.unpack('I', mdebug_section.data[9*4 : 10*4])
            cb_ss_offset, = fmt.unpack('I', mdebug_section.data[15*4 : 16*4])
            for i in range(ifd_max):
                offset = cb_fd_offset + 18*4*i
                iss_base, _, isym_base, csym = fmt.unpack('IIII', objfile.data[offset + 2*4 : offset + 6*4])
                scope_level = 0
                for j in range(csym):
                    offset2 = cb_sym_offset + 12 * (isym_base + j)
                    iss, value, st_sc_index = fmt.unpack('III', objfile.data[offset2 : offset2 + 12])
                    st = (st_sc_index >> 26)
                    sc = (st_sc_index >> 21) & 0x1f
                    if st in (MIPS_DEBUG_ST_STATIC, MIPS_DEBUG_ST_STATIC_PROC):
                        symbol_name_offset = cb_ss_offset + iss_base + iss
                        symbol_name_offset_end = objfile.data.find(b'\0', symbol_name_offset)
                        assert symbol_name_offset_end != -1
                        symbol_name = objfile.data[symbol_name_offset : symbol_name_offset_end]
                        if scope_level > 1:
                            # For in-function statics, append an increasing counter to
                            # the name, to avoid duplicate conflicting symbols.
                            count = static_name_count.get(symbol_name, 0) + 1
                            static_name_count[symbol_name] = count
                            symbol_name += b":" + str(count).encode("utf-8")
                        emitted_symbol_name = symbol_name
                        if convert_statics == "global-with-filename":
                            # Change the emitted symbol name to include the filename,
                            # but don't let that affect deduplication logic (we still
                            # want to be able to reference statics from GLOBAL_ASM).
                            emitted_symbol_name = objfile_name.encode("utf-8") + b":" + symbol_name
                        section_name = {1: '.text', 2: '.data', 3: '.bss', 15: '.rodata'}[sc]
                        section = objfile.find_section(section_name)
                        symtype = STT_FUNC if sc == 1 else STT_OBJECT
                        binding = STB_GLOBAL if make_statics_global else STB_LOCAL
                        sym = Symbol.from_parts(
                            fmt,
                            st_name=strtab_index,
                            st_value=value,
                            st_size=0,
                            st_info=(binding << 4 | symtype),
                            st_other=STV_DEFAULT,
                            st_shndx=section.index,
                            strtab=objfile.symtab.strtab,
                            name=symbol_name.decode('latin1'))
                        strtab_index += len(emitted_symbol_name) + 1
                        new_strtab_data.append(emitted_symbol_name + b'\0')
                        new_syms.append(sym)
                    if st in (
                        MIPS_DEBUG_ST_FILE,
                        MIPS_DEBUG_ST_STRUCT,
                        MIPS_DEBUG_ST_UNION,
                        MIPS_DEBUG_ST_ENUM,
                        MIPS_DEBUG_ST_BLOCK,
                        MIPS_DEBUG_ST_PROC,
                        MIPS_DEBUG_ST_STATIC_PROC,
                    ):
                        scope_level += 1
                    if st == MIPS_DEBUG_ST_END:
                        scope_level -= 1
                assert scope_level == 0
            objfile.symtab.strtab.data += b''.join(new_strtab_data)

        # Get rid of duplicate symbols, favoring ones that are not UNDEF.
        # Skip this for unnamed local symbols though.
        new_syms.sort(key=lambda s: 0 if s.st_shndx != SHN_UNDEF else 1)
        old_syms = []
        newer_syms = []
        name_to_sym = {}
        for s in new_syms:
            if s.name == "_gp_disp":
                s.type = STT_OBJECT
            if s.bind == STB_LOCAL and s.st_shndx == SHN_UNDEF:
                raise Failure("local symbol \"" + s.name + "\" is undefined")
            if not s.name:
                if s.bind != STB_LOCAL:
                    raise Failure("global symbol with no name")
                newer_syms.append(s)
            else:
                existing = name_to_sym.get(s.name)
                if not existing:
                    name_to_sym[s.name] = s
                    newer_syms.append(s)
                elif s.st_shndx != SHN_UNDEF and not (
                    existing.st_shndx == s.st_shndx and existing.st_value == s.st_value
                ):
                    raise Failure("symbol \"" + s.name + "\" defined twice")
                else:
                    s.replace_by = existing
                    old_syms.append(s)
        new_syms = newer_syms

        # Put local symbols in front, with the initial dummy entry first, and
        # _gp_disp at the end if it exists.
        new_syms.insert(0, empty_symbol)
        new_syms.sort(key=lambda s: (s.bind != STB_LOCAL, s.name == "_gp_disp"))
        num_local_syms = sum(1 for s in new_syms if s.bind == STB_LOCAL)

        for i, s in enumerate(new_syms):
            s.new_index = i
        for s in old_syms:
            s.new_index = s.replace_by.new_index
        objfile.symtab.data = b''.join(s.to_bin() for s in new_syms)
        objfile.symtab.sh_info = num_local_syms

        # Fix up relocation symbol references
        for sectype in SECTIONS:
            target = objfile.find_section(sectype)

            if target is not None:
                # fixup relocation symbol indices, since we butchered them above
                for reltab in target.relocated_by:
                    nrels = []
                    for rel in reltab.relocations:
                        if (sectype == '.text' and rel.r_offset in modified_text_positions or
                            sectype == '.rodata' and rel.r_offset in jtbl_rodata_positions):
                            # don't include relocations for late_rodata dummy code
                            continue
                        rel.sym_index = objfile.symtab.symbol_entries[rel.sym_index].new_index
                        nrels.append(rel)
                    reltab.relocations = nrels
                    reltab.data = b''.join(rel.to_bin() for rel in nrels)

        # Move over relocations
        for sectype in SECTIONS + ['.late_rodata']:
            source = asm_objfile.find_section(sectype)
            if source is None or not source.data:
                continue

            target_sectype = '.rodata' if sectype == '.late_rodata' else sectype
            target = objfile.find_section(target_sectype)
            assert target is not None, target_sectype
            target_reltab = objfile.find_section('.rel' + target_sectype)
            target_reltaba = objfile.find_section('.rela' + target_sectype)
            for reltab in source.relocated_by:
                for rel in reltab.relocations:
                    rel.sym_index = asm_objfile.symtab.symbol_entries[rel.sym_index].new_index
                    if sectype == '.late_rodata':
                        rel.r_offset = moved_late_rodata[rel.r_offset]
                new_data = b''.join(rel.to_bin() for rel in reltab.relocations)
                if reltab.sh_type == SHT_REL:
                    if not target_reltab:
                        target_reltab = objfile.add_section('.rel' + target_sectype,
                                sh_type=SHT_REL, sh_flags=0,
                                sh_link=objfile.symtab.index, sh_info=target.index,
                                sh_addralign=4, sh_entsize=8, data=b'')
                    target_reltab.data += new_data
                else:
                    if not target_reltaba:
                        target_reltaba = objfile.add_section('.rela' + target_sectype,
                                sh_type=SHT_RELA, sh_flags=0,
                                sh_link=objfile.symtab.index, sh_info=target.index,
                                sh_addralign=4, sh_entsize=12, data=b'')
                    target_reltaba.data += new_data

        objfile.write(objfile_name)
    finally:
        s_file.close()
        os.remove(s_name)
        try:
            os.remove(o_name)
        except:
            pass

def run_wrapped(argv, outfile, functions):
    parser = argparse.ArgumentParser(description="Pre-process .c files and post-process .o files to enable embedding assembly into C.")
    parser.add_argument('filename', help="path to .c code")
    parser.add_argument('--post-process', dest='objfile', help="path to .o file to post-process")
    parser.add_argument('--assembler', dest='assembler', help="assembler command (e.g. \"mips-linux-gnu-as -march=vr4300 -mabi=32\")")
    parser.add_argument('--asm-prelude', dest='asm_prelude', help="path to a file containing a prelude to the assembly file (with .set and .macro directives, e.g.)")
    parser.add_argument('--input-enc', default='latin1', help="input encoding (default: %(default)s)")
    parser.add_argument('--output-enc', default='latin1', help="output encoding (default: %(default)s)")
    parser.add_argument('--drop-mdebug-gptab', dest='drop_mdebug_gptab', action='store_true', help="drop mdebug and gptab sections")
    parser.add_argument('--convert-statics', dest='convert_statics', choices=["no", "local", "global", "global-with-filename"], default="local", help="change static symbol visibility (default: %(default)s)")
    parser.add_argument('--force', dest='force', action='store_true', help="force processing of files without GLOBAL_ASM blocks")
    parser.add_argument('--encode-cutscene-data-floats', dest='enable_cutscene_data_float_encoding', action='store_true', default=False, help="Replace floats with their encoded hexadecimal representation in CutsceneData data")
    parser.add_argument('-framepointer', dest='framepointer', action='store_true')
    parser.add_argument('-mips1', dest='mips1', action='store_true')
    parser.add_argument('-g3', dest='g3', action='store_true')
    parser.add_argument('-KPIC', dest='kpic', action='store_true')
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('-O0', dest='opt', action='store_const', const='O0')
    group.add_argument('-O1', dest='opt', action='store_const', const='O1')
    group.add_argument('-O2', dest='opt', action='store_const', const='O2')
    group.add_argument('-g', dest='opt', action='store_const', const='g')
    args = parser.parse_args(argv)
    opt = args.opt
    pascal = any(args.filename.endswith(ext) for ext in (".p", ".pas", ".pp"))
    if args.g3:
        if opt != 'O2':
            raise Failure("-g3 is only supported together with -O2")
        opt = 'g3'
    if args.mips1 and (opt not in ('O1', 'O2') or args.framepointer):
        raise Failure("-mips1 is only supported together with -O1 or -O2")
    if pascal and opt not in ('O1', 'O2', 'g3'):
        raise Failure("Pascal is only supported together with -O1, -O2 or -O2 -g3")
    opts = Opts(opt, args.framepointer, args.mips1, args.kpic, pascal, args.input_enc, args.output_enc, args.enable_cutscene_data_float_encoding)

    if args.objfile is None:
        with open(args.filename, encoding=args.input_enc) as f:
            deps = []
            functions = parse_source(f, opts, out_dependencies=deps, print_source=outfile)
            return functions, deps
    else:
        if args.assembler is None:
            raise Failure("must pass assembler command")
        if functions is None:
            with open(args.filename, encoding=args.input_enc) as f:
                functions = parse_source(f, opts, out_dependencies=[])
        if not functions and not args.force:
            return
        asm_prelude = b''
        if args.asm_prelude:
            with open(args.asm_prelude, 'rb') as f:
                asm_prelude = f.read()
        fixup_objfile(args.objfile, functions, asm_prelude, args.assembler, args.output_enc, args.drop_mdebug_gptab, args.convert_statics)

def run(argv, outfile=sys.stdout.buffer, functions=None):
    try:
        return run_wrapped(argv, outfile, functions)
    except Failure as e:
        print("Error:", e, file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    run(sys.argv[1:])
