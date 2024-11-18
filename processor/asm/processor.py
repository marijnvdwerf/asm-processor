import re
import os
import sys
import struct
import argparse
import tempfile
from collections import namedtuple
from io import StringIO

from ..utils.errors import Failure
from ..utils.state import GlobalState
from ..utils.options import Opts
from ..elf.file import ElfFile
from ..elf.symbol import STB_GLOBAL, STV_HIDDEN
from ..elf.section import SHT_PROGBITS, SHF_ALLOC, SHF_WRITE, SHF_EXECINSTR
from .block import GlobalAsmBlock
from .function import Function

MAX_FN_SIZE = 100
SLOW_CHECKS = False

cutscene_data_regexpr = re.compile(r"CutsceneData (.|\n)*\[\] = {")
float_regexpr = re.compile(r"[-+]?[0-9]*\.?[0-9]+([eE][-+]?[0-9]+)?f")

def repl_float_hex(m):
    return str(int.from_bytes(struct.pack(">f", float(m.group())), byteorder="big"))

def parse_source(f, opts, out_dependencies, print_source=None):
    opt = opts.opt
    framepointer = opts.framepointer
    mips1 = opts.mips1
    kpic = opts.kpic
    pascal = opts.pascal
    input_enc = opts.input_enc
    output_enc = opts.output_enc
    enable_cutscene_data_float_encoding = opts.enable_cutscene_data_float_encoding

    min_instr_count = 2 if opt in ['O2', 'O1'] else 0
    skip_instr_count = 4 if opt == 'O2' else 0
    if framepointer:
        min_instr_count += 1
        skip_instr_count += 1
    state = GlobalState(min_instr_count, skip_instr_count, opt == 'O2', opt in ['O2', 'O1'], mips1, pascal)

    global_asm = None
    asm_functions = []
    output_lines = []

    is_cutscene_data = False
    is_late_rodata = False
    if_elif_count = 0

    for line in f:
        if print_source:
            print_source.write(line)
        
        line = line.rstrip()
        original_line = line

        if line.startswith('#include "') and line.endswith('.s"'):
            dep = line[10:-3]
            out_dependencies.append(dep)
            continue

        if enable_cutscene_data_float_encoding:
            if cutscene_data_regexpr.search(line):
                is_cutscene_data = True
            if is_cutscene_data and float_regexpr.search(line):
                line = float_regexpr.sub(repl_float_hex, line)

        if global_asm is not None:
            if line.strip() == '}':
                src = global_asm.finish(state)
                for s in src:
                    output_lines.append(s)
                if len(global_asm.fn_section_sizes['.text']) > MAX_FN_SIZE:
                    raise Failure(f"function too big ({len(global_asm.fn_section_sizes['.text'])} bytes / {MAX_FN_SIZE} bytes)")
                asm_functions.append(Function(
                    global_asm.text_glabels,
                    global_asm.asm_conts,
                    global_asm.late_rodata_dummy_bytes,
                    global_asm.jtbl_rodata_size,
                    global_asm.late_rodata_asm_conts,
                    global_asm.fn_desc,
                    global_asm.data))
                global_asm = None
                if_elif_count = 0
            else:
                global_asm.process_line(line, output_enc)
        else:
            if line.startswith('GLOBAL_ASM('):
                global_asm = GlobalAsmBlock(('GLOBAL_ASM', None))
            elif line.startswith('#pragma GLOBAL_ASM('):
                global_asm = GlobalAsmBlock(('#pragma', None))
            elif line.startswith('INCLUDE_ASM(') or line.startswith('INCLUDE_RODATA('):
                fname = line[line.find('(') + 1 : line.find(')')].strip()
                if ',' in fname:
                    fname = fname.split(',')[1].strip()
                fname = f"{fname}.s"
                out_dependencies.append(fname)
                continue
            elif line.startswith('#ifdef') or line.startswith('#ifndef'):
                if_elif_count += 1
            elif line.startswith('#elif') or line.startswith('#else'):
                if if_elif_count > 0:
                    pass # ok
            elif line.startswith('#endif'):
                if if_elif_count > 0:
                    if_elif_count -= 1
            elif line.startswith('#define') or line.startswith('#undef') or line.startswith('#include'):
                pass # ok
            else:
                output_lines.append(line)

    if global_asm is not None:
        raise Failure("unterminated .global_asm")

    return asm_functions, output_lines

def fixup_objfile(objfile_name, functions, asm_prelude, assembler, output_enc, drop_mdebug_gptab, convert_statics):
    with open(objfile_name, 'rb') as f:
        objfile = ElfFile(f.read())

    if convert_statics:
        # Convert all function static variables to global ones
        # This is a hack to make the function work as expected without having to convert all instances
        # of la/ld to la.u/la.l/lw in the assembly
        for s in objfile.sections:
            for sym in s.local_symbols():
                if not sym.name.startswith('.L'):
                    sym.bind = STB_GLOBAL
                    sym.visibility = STV_HIDDEN

    if drop_mdebug_gptab:
        objfile.drop_mdebug_gptab()

    # Add section data
    local_text_data = []
    local_data_data = []
    local_rodata_data = []
    late_rodata_data = []
    late_rodata_source = []

    for (text_glabels, asm_conts, late_rodata_dummy_bytes, jtbl_rodata_size, late_rodata_asm_conts, fn_desc, data) in functions:
        for line in asm_conts:
            local_text_data.append(line.encode(output_enc) + b'\n')
        if data:
            for line in data:
                local_data_data.append(line.encode(output_enc) + b'\n')
        for line in late_rodata_asm_conts:
            late_rodata_data.append(line.encode(output_enc) + b'\n')
        late_rodata_source.append((late_rodata_dummy_bytes, jtbl_rodata_size, fn_desc, text_glabels))

    # Move .rodata to .data
    objfile.add_section('.data', SHT_PROGBITS, SHF_ALLOC | SHF_WRITE, 0, 0, 4, 0, local_data_data)

    # Add .text section
    local_text = objfile.add_section('.text', SHT_PROGBITS, SHF_ALLOC | SHF_EXECINSTR, 0, 0, 4, 0, local_text_data)

    # Move .rodata to .late_rodata
    objfile.add_section('.late_rodata', SHT_PROGBITS, SHF_ALLOC, 0, 0, 4, 0, late_rodata_data)

    # Unify segments
    segments = []
    for i, s in enumerate(objfile.sections):
        if s.sh_flags & SHF_ALLOC:
            is_text = bool(s.sh_flags & SHF_EXECINSTR)
            is_data = bool(s.sh_flags & SHF_WRITE)
            segments.append((is_text, is_data, s))

    segments.sort()
    for i, s in enumerate(segments):
        if i != len(segments) - 1:
            if segments[i + 1][2].sh_addr != s[2].sh_addr + s[2].sh_size:
                segments[i + 1][2].sh_addr = s[2].sh_addr + s[2].sh_size

    # Write the final file
    objfile.write(objfile_name)

def run_wrapped(argv, outfile, functions):
    parser = argparse.ArgumentParser(description="Pre-process .c files and post-process .o files to enable embedding assembly into C source files")
    parser.add_argument('filename', help="path to input file")
    parser.add_argument('--drop-mdebug-gptab', dest='drop_mdebug_gptab', action='store_true', help="drop .mdebug and .gptab sections")
    parser.add_argument('--convert-statics', dest='convert_statics', action='store_true', help="convert static variables to global")
    parser.add_argument('--assembler', dest='assembler', help="assembler command (e.g. 'mips-linux-gnu-as -march=vr4300 -mabi=32')")
    parser.add_argument('--asm-prelude', dest='asm_prelude', help="path to asm prelude file")
    parser.add_argument('--input-enc', default='latin1', help="input encoding (default: latin1)")
    parser.add_argument('--output-enc', default='latin1', help="output encoding (default: latin1)")
    parser.add_argument('--output-dependencies', dest='output_dependencies', help="output make dependencies file")
    parser.add_argument('--enable-cutscene-data-float-encoding', dest='enable_cutscene_data_float_encoding', action='store_true', help="enable float encoding for cutscene data")
    args = parser.parse_args(argv)

    opts = Opts(
        opt='O2',
        framepointer=False,
        mips1=False,
        kpic=False,
        pascal=False,
        input_enc=args.input_enc,
        output_enc=args.output_enc,
        enable_cutscene_data_float_encoding=args.enable_cutscene_data_float_encoding)

    with open(args.filename, encoding=args.input_enc) as f:
        functions, output_lines = parse_source(f, opts, [], print_source=outfile)

    if args.output_dependencies:
        with open(args.output_dependencies, 'w') as f:
            f.write(f'{args.filename}: ')
            for dep in dependencies:
                f.write(f' \\\n    {dep}')
            f.write('\n')

    if functions:
        asm_prelude = None
        if args.asm_prelude:
            with open(args.asm_prelude) as f:
                asm_prelude = f.read()

        fixup_objfile(args.filename, functions, asm_prelude, args.assembler,
                     args.output_enc, args.drop_mdebug_gptab, args.convert_statics)

def run(argv, outfile=sys.stdout.buffer, functions=None):
    try:
        run_wrapped(argv, outfile, functions)
    except Failure as e:
        print("Error:", e.message, file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    run(sys.argv[1:])
