class GlobalState:
    def __init__(self, min_instr_count, skip_instr_count, use_jtbl_for_rodata, prelude_if_late_rodata, mips1, pascal):
        # A value that hopefully never appears as a 32-bit rodata constant (or we
        # miscompile late rodata). Increases by 1 in each step.
        self.late_rodata_hex = 0xE0123456
        self.valuectr = 0
        self.namectr = 0
        self.min_instr_count = min_instr_count
        self.skip_instr_count = skip_instr_count
        self.use_jtbl_for_rodata = use_jtbl_for_rodata
        self.prelude_if_late_rodata = prelude_if_late_rodata
        self.mips1 = mips1
        self.pascal = pascal

    def next_late_rodata_hex(self):
        val = self.late_rodata_hex
        self.late_rodata_hex += 1
        return val

    def make_name(self, cat):
        self.namectr += 1
        return f'_asmpp_{cat}_{self.namectr}'

    def func_prologue(self, name):
        if self.pascal:
            return f'PROCEDURE {name}; [alias]; asregs;'
        else:
            return f'void {name}(void) {{ __asm__('

    def func_epilogue(self):
        if self.pascal:
            return 'END;'
        else:
            return '); }'

    def pascal_assignment(self, tp, val):
        if tp == 'float':
            return f'CONST REAL = {val};'
        else:
            return f'CONST INTEGER = {val};'
