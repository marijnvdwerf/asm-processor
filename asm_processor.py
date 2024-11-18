#!/usr/bin/env python3
import argparse
import sys

from processor.utils.errors import Failure
from processor.objfile import fixup_objfile
from processor.processor import parse_source

from processor.utils.options import Opts

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
