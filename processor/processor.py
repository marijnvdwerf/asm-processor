import re
import struct
import os
from io import StringIO

from .utils.state import GlobalState
from .utils.errors import Failure
from .asm.block import GlobalAsmBlock

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
