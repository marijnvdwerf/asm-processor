import struct

class GlobalState:
    def __init__(self, min_instr_count: int, skip_instr_count: int, use_jtbl_for_rodata: bool, prelude_if_late_rodata: int, mips1: bool, pascal: bool) -> None:
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

    def next_late_rodata_hex(self) -> bytes:
        dummy_bytes = struct.pack('>I', self.late_rodata_hex)
        if (self.late_rodata_hex & 0xffff) == 0:
            # Avoid lui
            self.late_rodata_hex += 1
        self.late_rodata_hex += 1
        return dummy_bytes

    def make_name(self, cat: str) -> str:
        self.namectr += 1
        return '_asmpp_{}{}'.format(cat, self.namectr)

    def func_prologue(self, name: str) -> str:
        if self.pascal:
            return " ".join([
                "procedure {}();".format(name),
                "type",
                " pi = ^integer;",
                " pf = ^single;",
                " pd = ^double;",
                "var",
                " vi: pi;",
                " vf: pf;",
                " vd: pd;",
                "begin",
                " vi := vi;",
                " vf := vf;",
                " vd := vd;",
            ])
        else:
            return 'void {}(void) {{'.format(name)

    def func_epilogue(self) -> str:
        if self.pascal:
            return "end;"
        else:
            return "}"

    def pascal_assignment(self, tp: str, val: str) -> str:
        self.valuectr += 1
        address = (8 * self.valuectr) & 0x7FFF
        return 'v{} := p{}({}); v{}^ := {};'.format(tp, tp, address, tp, val)

