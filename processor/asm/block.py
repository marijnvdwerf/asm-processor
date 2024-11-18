import re
from ..utils.errors import Failure

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
        if line:
            message = f'Line {self.num_lines}: {message}'
        raise Failure(message)

    def count_quoted_size(self, line, z, real_line, output_enc):
        line = line.encode(output_enc).decode('latin1')
        in_quote = False
        num_parts = 0
        ret = 0
        i = 0
        digits = "0123456789"
        while i < len(line):
            c = line[i]
            i += 1
            ret += 1
            if not in_quote:
                if c == '"':
                    in_quote = True
                    num_parts += 1
            else:
                if c == '"':
                    in_quote = False
                    if z and num_parts == 1:
                        ret += 1
                elif c == '\\':
                    if i == len(line):
                        self.fail("backslash at end of line not supported", real_line)
                    c = line[i]
                    i += 1
                    # (if c is in "nrt\\\"", we have a single-char escape sequence)
                    if c == 'x':
                        if i >= len(line) - 1:
                            self.fail("\\x used with no following hex digits", real_line)
                        d1 = line[i]
                        d2 = line[i+1]
                        i += 2
                        if d1 not in digits + "abcdefABCDEF" or d2 not in digits + "abcdefABCDEF":
                            self.fail("\\x used with invalid hex digits", real_line)
                    elif c == '0':
                        ret -= 1
                    elif c not in "nrt\\\""":
                        self.fail(f"unknown escape \\{c}", real_line)
        return ret

    def align2(self):
        while self.fn_section_sizes[self.cur_section] % 2 != 0:
            self.fn_section_sizes[self.cur_section] += 1

    def align4(self):
        while self.fn_section_sizes[self.cur_section] % 4 != 0:
            self.fn_section_sizes[self.cur_section] += 1

    def add_sized(self, size, line):
        if self.cur_section in ['.text', '.late_rodata']:
            if size % 4 != 0:
                self.fail(f"size must be a multiple of 4 for .text/.late_rodata", line)
        self.fn_section_sizes[self.cur_section] += size
        if self.cur_section == '.text':
            if line is not None:
                self.fn_ins_inds.append((self.fn_section_sizes['.text'], line))
            for _ in range(0, size, 4):
                self.asm_conts.append('.word 0')
        elif self.cur_section == '.late_rodata':
            if size == 0:
                return
            if not self.late_rodata_alignment:
                self.late_rodata_alignment = 4
                self.late_rodata_alignment_from_content = True
            if size < 4:
                self.fail(".late_rodata contents must be at least 4 bytes", line)
            for _ in range(0, size, 4):
                self.late_rodata_asm_conts.append('.word 0')

    def process_line(self, line, output_enc):
        self.num_lines += 1
        if line.endswith('\\'):
            self.glued_line += line[:-1]
            return
        line = self.glued_line + line
        self.glued_line = ''

        real_line = line
        line = re.sub(re.compile(r'/\*.*?\*/|//.*$'), '', line)
        line = line.strip()
        if not line:
            return

        if line.startswith('glabel '):
            self.text_glabels.append(line.split()[1])
        if line.startswith('@'):
            self.fail("'@' at beginning of line only allowed after label", real_line)
        if line.startswith('glabel ') or line.startswith('.'):
            self.asm_conts.append(line)
        elif line.startswith('/*'):
            self.fail("inline comments not supported", real_line)
        elif line.startswith('#'):
            self.fail("# not supported at beginning of line (use // or /* */ instead)", real_line)
        elif line.startswith('@'):
            self.fail("misplaced @", real_line)
        else:
            self.fail("instruction or label expected", real_line)

    def finish(self, state):
        src = []
        late_rodata = []
        late_rodata_fn_output = []

        if self.fn_section_sizes['.late_rodata'] > 0:
            # Generate late rodata by emitting code that has the same size as the
            # late_rodata data. This can be either code that writes to a static
            # variable (+ -g) or code that writes to a global (-g).
            # Generally, we want to do the former, which lets us combine static
            # and non-static data in the same function. However, this doesn't work
            # in all cases:
            # - If the output doesn't use %hi/%lo (i.e. is position-independent),
            #   we can't write to static memory, since that requires an absolute
            #   memory reference.
            # - If the output is -g and we have multiple late_rodata symbols, we
            #   can't write to static memory, since that would require jumping to
            #   absolute addresses.
            # - If the output is -g and we have a jump table (which requires
            #   late_rodata), we can't write to static memory since that would
            #   overwrite the jump table.
            size = self.fn_section_sizes['.late_rodata']
            if (not state.use_jtbl_for_rodata and
                    (not state.prelude_if_late_rodata or size == 4)):
                # Use static writes
                for i in range(0, size, 4):
                    late_rodata.append(f'.word 0x{state.next_late_rodata_hex():X}')
                    src.append(f'*(volatile unsigned int *)(&{state.make_name("Ldata")} + {i}) = '
                             f'0x{state.late_rodata_hex - 1:X};')
            else:
                # Use global writes
                sym = state.make_name('Lrodata')
                late_rodata.append(f'glabel {sym}')
                for i in range(0, size, 4):
                    late_rodata.append(f'.word 0x{state.next_late_rodata_hex():X}')
                    if state.mips1:
                        src.extend([
                            f'lui $at, %hi({sym} + {i})',
                            f'sw $t9, %lo({sym} + {i})($at)',
                        ])
                    else:
                        src.append(f'sw $t9, %lo({sym} + {i})(%hi({sym} + {i}))')

        rodata_name = None
        if self.fn_section_sizes['.rodata'] > 0 or late_rodata:
            rodata_name = state.make_name('Lrodata')
            src.append(f'glabel {rodata_name}')
            if self.fn_section_sizes['.rodata'] > 0:
                src.append('.word 0')
            src.extend(late_rodata)

        src.append('.set reorder')
        src.append('.end macro')
        src.append('')
        src.append('.macro .late_rodata')
        src.extend(late_rodata_fn_output)
        src.append('.end_macro')

        return src
