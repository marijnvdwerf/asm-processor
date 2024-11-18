import re
from .function import Function
from ..utils.errors import Failure
from ..utils.constants import MAX_FN_SIZE
import struct


# https://stackoverflow.com/a/241506
def re_comment_replacer(match):
    s = match.group(0)
    if s[0] in "/#":
        return " "
    else:
        return s


re_comment_or_string = re.compile(
    r'#.*|/\*.*?\*/|"(?:\\.|[^\\"])*"'
)


class GlobalAsmBlock:
    def __init__(self, fn_desc):
        self.fn_desc = fn_desc
        self.cur_section = '.text'
        self.asm_conts = []
        self.late_rodata_asm_conts = []
        self.late_rodata_alignment = 0
        self.late_rodata_alignment_from_content = False
        self.text_glabels = []
        self.fn_section_sizes = {
            '.text': 0,
            '.data': 0,
            '.bss': 0,
            '.rodata': 0,
            '.late_rodata': 0,
        }
        self.fn_ins_inds = []
        self.glued_line = ''
        self.num_lines = 0

    def fail(self, message, line=None):
        context = self.fn_desc
        if line:
            context += ", at line \"" + line + "\""
        raise Failure(message + "\nwithin " + context)

    def count_quoted_size(self, line, z, real_line, output_enc):
        line = line.encode(output_enc).decode('latin1')
        in_quote = False
        has_comma = True
        num_parts = 0
        ret = 0
        i = 0
        digits = "0123456789" # 0-7 would be more sane, but this matches GNU as
        while i < len(line):
            c = line[i]
            i += 1
            if not in_quote:
                if c == '"':
                    in_quote = True
                    if z and not has_comma:
                        self.fail(".asciiz with glued strings is not supported due to GNU as version diffs")
                    num_parts += 1
                elif c == ',':
                    has_comma = True
            else:
                if c == '"':
                    in_quote = False
                    has_comma = False
                    continue
                ret += 1
                if c != '\\':
                    continue
                if i == len(line):
                    self.fail("backslash at end of line not supported", real_line)
                c = line[i]
                i += 1
                # (if c is in "bfnrtv", we have a real escaped literal)
                if c == 'x':
                    # hex literal, consume any number of hex chars, possibly none
                    while i < len(line) and line[i] in digits + "abcdefABCDEF":
                        i += 1
                elif c in digits:
                    # octal literal, consume up to two more digits
                    it = 0
                    while i < len(line) and line[i] in digits and it < 2:
                        i += 1
                        it += 1

        if in_quote:
            self.fail("unterminated string literal", real_line)
        if num_parts == 0:
            self.fail(".ascii with no string", real_line)
        return ret + num_parts if z else ret

    def align2(self):
        while self.fn_section_sizes[self.cur_section] % 2 != 0:
            self.fn_section_sizes[self.cur_section] += 1

    def align4(self):
        while self.fn_section_sizes[self.cur_section] % 4 != 0:
            self.fn_section_sizes[self.cur_section] += 1

    def add_sized(self, size, line):
        if self.cur_section in ['.text', '.late_rodata']:
            if size % 4 != 0:
                self.fail("size must be a multiple of 4", line)
        if size < 0:
            self.fail("size cannot be negative", line)
        self.fn_section_sizes[self.cur_section] += size
        if self.cur_section == '.text':
            if not self.text_glabels:
                self.fail(".text block without an initial glabel", line)
            self.fn_ins_inds.append((self.num_lines - 1, size // 4))

    def process_line(self, line, output_enc):
        self.num_lines += 1
        if line.endswith('\\'):
            self.glued_line += line[:-1]
            return
        line = self.glued_line + line
        self.glued_line = ''

        real_line = line
        line = re.sub(re_comment_or_string, re_comment_replacer, line)
        line = line.strip()
        line = re.sub(r'^[a-zA-Z0-9_]+:\s*', '', line)
        changed_section = False
        emitting_double = False
        if (line.startswith('glabel ') or line.startswith('jlabel ')) and self.cur_section == '.text':
            self.text_glabels.append(line.split()[1])
        if not line:
            pass # empty line
        elif line.startswith('glabel ') or line.startswith('dlabel ') or line.startswith('jlabel ') or line.startswith('endlabel ') or (' ' not in line and line.endswith(':')):
            pass # label
        elif line.startswith('.section') or line in ['.text', '.data', '.rdata', '.rodata', '.bss', '.late_rodata']:
            # section change
            self.cur_section = '.rodata' if line == '.rdata' else line.split(',')[0].split()[-1]
            if self.cur_section not in ['.data', '.text', '.rodata', '.late_rodata', '.bss']:
                self.fail("unrecognized .section directive", real_line)
            changed_section = True
        elif line.startswith('.late_rodata_alignment'):
            if self.cur_section != '.late_rodata':
                self.fail(".late_rodata_alignment must occur within .late_rodata section", real_line)
            value = int(line.split()[1])
            if value not in [4, 8]:
                self.fail(".late_rodata_alignment argument must be 4 or 8", real_line)
            if self.late_rodata_alignment and self.late_rodata_alignment != value:
                self.fail(".late_rodata_alignment alignment assumption conflicts with earlier .double directive. Make sure to provide explicit alignment padding.")
            self.late_rodata_alignment = value
            changed_section = True
        elif line.startswith('.incbin'):
            self.add_sized(int(line.split(',')[-1].strip(), 0), real_line)
        elif line.startswith('.word') or line.startswith('.gpword') or line.startswith('.float'):
            self.align4()
            self.add_sized(4 * len(line.split(',')), real_line)
        elif line.startswith('.double'):
            self.align4()
            if self.cur_section == '.late_rodata':
                align8 = self.fn_section_sizes[self.cur_section] % 8
                # Automatically set late_rodata_alignment, so the generated C code uses doubles.
                # This gives us correct alignment for the transferred doubles even when the
                # late_rodata_alignment is wrong, e.g. for non-matching compilation.
                if not self.late_rodata_alignment:
                    self.late_rodata_alignment = 8 - align8
                    self.late_rodata_alignment_from_content = True
                elif self.late_rodata_alignment != 8 - align8:
                    if self.late_rodata_alignment_from_content:
                        self.fail("found two .double directives with different start addresses mod 8. Make sure to provide explicit alignment padding.", real_line)
                    else:
                        self.fail(".double at address that is not 0 mod 8 (based on .late_rodata_alignment assumption). Make sure to provide explicit alignment padding.", real_line)
            self.add_sized(8 * len(line.split(',')), real_line)
            emitting_double = True
        elif line.startswith('.space'):
            self.add_sized(int(line.split()[1], 0), real_line)
        elif line.startswith('.balign'):
            align = int(line.split()[1])
            if align != 4:
                self.fail("only .balign 4 is supported", real_line)
            self.align4()
        elif line.startswith('.align'):
            align = int(line.split()[1])
            if align != 2:
                self.fail("only .align 2 is supported", real_line)
            self.align4()
        elif line.startswith('.asci'):
            z = (line.startswith('.asciz') or line.startswith('.asciiz'))
            self.add_sized(self.count_quoted_size(line, z, real_line, output_enc), real_line)
        elif line.startswith('.byte'):
            self.add_sized(len(line.split(',')), real_line)
        elif line.startswith('.half') or line.startswith('.hword') or line.startswith(".short"):
            self.align2()
            self.add_sized(2*len(line.split(',')), real_line)
        elif line.startswith('.size'):
            pass
        elif line.startswith('.'):
            # .macro, ...
            self.fail("asm directive not supported", real_line)
        else:
            # Unfortunately, macros are hard to support for .rodata --
            # we don't know how how space they will expand to before
            # running the assembler, but we need that information to
            # construct the C code. So if we need that we'll either
            # need to run the assembler twice (at least in some rare
            # cases), or change how this program is invoked.
            # Similarly, we can't currently deal with pseudo-instructions
            # that expand to several real instructions.
            if self.cur_section != '.text':
                self.fail("instruction or macro call in non-.text section? not supported", real_line)
            self.add_sized(4, real_line)
        if self.cur_section == '.late_rodata':
            if not changed_section:
                if emitting_double:
                    self.late_rodata_asm_conts.append(".align 0")
                self.late_rodata_asm_conts.append(real_line)
                if emitting_double:
                    self.late_rodata_asm_conts.append(".align 2")
        else:
            self.asm_conts.append(real_line)

    def finish(self, state):
        src = [''] * (self.num_lines + 1)
        late_rodata_dummy_bytes = []
        jtbl_rodata_size = 0
        late_rodata_fn_output = []

        num_instr = self.fn_section_sizes['.text'] // 4

        if self.fn_section_sizes['.late_rodata'] > 0:
            # Generate late rodata by emitting unique float constants.
            # This requires 3 instructions for each 4 bytes of rodata.
            # If we know alignment, we can use doubles, which give 3
            # instructions for 8 bytes of rodata.
            size = self.fn_section_sizes['.late_rodata'] // 4
            skip_next = False
            needs_double = (self.late_rodata_alignment != 0)
            extra_mips1_nop = False
            if state.pascal:
                jtbl_size = 9 if state.mips1 else 8
                jtbl_min_rodata_size = 2
            else:
                jtbl_size = 11 if state.mips1 else 9
                jtbl_min_rodata_size = 5
            for i in range(size):
                if skip_next:
                    skip_next = False
                    continue
                # Jump tables give 9 instructions (11 with -mips1) for >= 5 words of rodata,
                # and should be emitted when:
                # - -O2 or -O2 -g3 are used, which give the right codegen
                # - we have emitted our first .float/.double (to ensure that we find the
                #   created rodata in the binary)
                # - we have emitted our first .double, if any (to ensure alignment of doubles
                #   in shifted rodata sections)
                # - we have at least 5 words of rodata left to emit (otherwise IDO does not
                #   generate a jump table)
                # - we have at least 10 more instructions to go in this function (otherwise our
                #   function size computation will be wrong since the delay slot goes unused)
                if (not needs_double and state.use_jtbl_for_rodata and i >= 1 and
                        size - i >= jtbl_min_rodata_size and
                        num_instr - len(late_rodata_fn_output) >= jtbl_size + 1):
                    if state.pascal:
                        cases = " ".join("{}: ;".format(case) for case in range(size - i))
                        line = "case 0 of " + cases + " otherwise end;"
                    else:
                        cases = " ".join("case {}:".format(case) for case in range(size - i))
                        line = "switch (*(volatile int*)0) { " + cases + " ; }"
                    late_rodata_fn_output.append(line)
                    late_rodata_fn_output.extend([""] * (jtbl_size - 1))
                    jtbl_rodata_size = (size - i) * 4
                    extra_mips1_nop = i != 2
                    break
                dummy_bytes = state.next_late_rodata_hex()
                late_rodata_dummy_bytes.append(dummy_bytes)
                if self.late_rodata_alignment == 4 * ((i + 1) % 2 + 1) and i + 1 < size:
                    dummy_bytes2 = state.next_late_rodata_hex()
                    late_rodata_dummy_bytes.append(dummy_bytes2)
                    fval, = struct.unpack('>d', dummy_bytes + dummy_bytes2)
                    if state.pascal:
                        line = state.pascal_assignment('d', fval)
                    else:
                        line = '*(volatile double*)0 = {};'.format(fval)
                    late_rodata_fn_output.append(line)
                    skip_next = True
                    needs_double = False
                    if state.mips1:
                        # mips1 does not have ldc1/sdc1
                        late_rodata_fn_output.append('')
                        late_rodata_fn_output.append('')
                    extra_mips1_nop = False
                else:
                    fval, = struct.unpack('>f', dummy_bytes)
                    if state.pascal:
                        line = state.pascal_assignment('f', fval)
                    else:
                        line = '*(volatile float*)0 = {}f;'.format(fval)
                    late_rodata_fn_output.append(line)
                    extra_mips1_nop = True
                late_rodata_fn_output.append('')
                late_rodata_fn_output.append('')
            if state.mips1 and extra_mips1_nop:
                late_rodata_fn_output.append('')

        text_name = None
        if self.fn_section_sizes['.text'] > 0 or late_rodata_fn_output:
            text_name = state.make_name('func')
            src[0] = state.func_prologue(text_name)
            src[self.num_lines] = state.func_epilogue()
            instr_count = self.fn_section_sizes['.text'] // 4
            if instr_count < state.min_instr_count:
                self.fail("too short .text block")
            tot_emitted = 0
            tot_skipped = 0
            fn_emitted = 0
            fn_skipped = 0
            skipping = True
            rodata_stack = late_rodata_fn_output[::-1]
            for (line, count) in self.fn_ins_inds:
                for _ in range(count):
                    if (fn_emitted > MAX_FN_SIZE and instr_count - tot_emitted > state.min_instr_count and
                            (not rodata_stack or rodata_stack[-1])):
                        # Don't let functions become too large. When a function reaches 284
                        # instructions, and -O2 -framepointer flags are passed, the IRIX
                        # compiler decides it is a great idea to start optimizing more.
                        # Also, Pascal cannot handle too large functions before it runs out
                        # of unique statements to write.
                        fn_emitted = 0
                        fn_skipped = 0
                        skipping = True
                        src[line] += (' ' + state.func_epilogue() + ' ' +
                            state.func_prologue(state.make_name('large_func')) + ' ')
                    if (
                        skipping and
                        fn_skipped < state.skip_instr_count +
                            (state.prelude_if_late_rodata if rodata_stack else 0)
                    ):
                        fn_skipped += 1
                        tot_skipped += 1
                    else:
                        skipping = False
                        if rodata_stack:
                            src[line] += rodata_stack.pop()
                        elif state.pascal:
                            src[line] += state.pascal_assignment('i', '0')
                        else:
                            src[line] += '*(volatile int*)0 = 0;'
                    tot_emitted += 1
                    fn_emitted += 1
            if rodata_stack:
                size = len(late_rodata_fn_output) // 3
                available = instr_count - tot_skipped
                self.fail(
                    "late rodata to text ratio is too high: {} / {} must be <= 1/3\n"
                    "add .late_rodata_alignment (4|8) to the .late_rodata "
                    "block to double the allowed ratio."
                        .format(size, available))

        rodata_name = None
        if self.fn_section_sizes['.rodata'] > 0:
            if state.pascal:
                self.fail(".rodata isn't supported with Pascal for now")
            rodata_name = state.make_name('rodata')
            src[self.num_lines] += ' const char {}[{}] = {{1}};'.format(rodata_name, self.fn_section_sizes['.rodata'])

        data_name = None
        if self.fn_section_sizes['.data'] > 0:
            data_name = state.make_name('data')
            if state.pascal:
                line = ' var {}: packed array[1..{}] of char := [otherwise: 0];'.format(data_name, self.fn_section_sizes['.data'])
            else:
                line = ' char {}[{}] = {{1}};'.format(data_name, self.fn_section_sizes['.data'])
            src[self.num_lines] += line

        bss_name = None
        if self.fn_section_sizes['.bss'] > 0:
            if state.pascal:
                self.fail(".bss isn't supported with Pascal")
            bss_name = state.make_name('bss')
            src[self.num_lines] += ' char {}[{}];'.format(bss_name, self.fn_section_sizes['.bss'])

        fn = Function(
                text_glabels=self.text_glabels,
                asm_conts=self.asm_conts,
                late_rodata_dummy_bytes=late_rodata_dummy_bytes,
                jtbl_rodata_size=jtbl_rodata_size,
                late_rodata_asm_conts=self.late_rodata_asm_conts,
                fn_desc=self.fn_desc,
                data={
                    '.text': (text_name, self.fn_section_sizes['.text']),
                    '.data': (data_name, self.fn_section_sizes['.data']),
                    '.rodata': (rodata_name, self.fn_section_sizes['.rodata']),
                    '.bss': (bss_name, self.fn_section_sizes['.bss']),
                })
        return src, fn
