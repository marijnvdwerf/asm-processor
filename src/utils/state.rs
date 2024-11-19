#[derive(Debug, Clone)]
pub struct GlobalState {
    pub min_instr_count: usize,
    pub skip_instr_count: usize,
    pub use_jtbl_for_rodata: bool,
    pub prelude_if_late_rodata: usize,
    pub mips1: bool,
    pub pascal: bool,
    // Internal state
    late_rodata_hex: u32,
    valuectr: u32,
    namectr: u32,
}

impl GlobalState {
    pub fn new(
        min_instr_count: usize,
        skip_instr_count: usize,
        use_jtbl_for_rodata: bool,
        prelude_if_late_rodata: usize,
        mips1: bool,
        pascal: bool,
    ) -> Self {
        Self {
            min_instr_count,
            skip_instr_count,
            use_jtbl_for_rodata,
            prelude_if_late_rodata,
            mips1,
            pascal,
            // Initialize internal state
            late_rodata_hex: 0xE0123456,
            valuectr: 0,
            namectr: 0,
        }
    }

    pub fn from_opts(opt: &str, framepointer: bool, mips1: bool, kpic: bool, pascal: bool) -> Self {
        let (mut min_instr_count, mut skip_instr_count) = match (opt, framepointer) {
            ("O1" | "O2", true) => (6, 5),
            ("O1" | "O2", false) => (2, 1),
            ("O0", true) => (8, 8),
            ("O0", false) => (4, 4),
            ("g", true) => (7, 7),
            ("g", false) => (4, 4),
            ("g3", true) => (4, 4),
            ("g3", false) => (2, 2),
            _ => panic!("Invalid optimization level"),
        };

        let prelude_if_late_rodata = if kpic {
            if opt == "g3" || opt == "O2" {
                3
            } else {
                min_instr_count += 3;
                skip_instr_count += 3;
                0
            }
        } else {
            0
        };

        let use_jtbl_for_rodata = matches!(opt, "O2" | "g3") && !framepointer && !kpic;

        Self::new(
            min_instr_count,
            skip_instr_count,
            use_jtbl_for_rodata,
            prelude_if_late_rodata,
            mips1,
            pascal,
        )
    }

    pub fn next_late_rodata_hex(&mut self) -> [u8; 4] {
        let dummy_bytes = self.late_rodata_hex.to_be_bytes();
        if (self.late_rodata_hex & 0xffff) == 0 {
            // Avoid lui
            self.late_rodata_hex += 1;
        }
        self.late_rodata_hex += 1;
        dummy_bytes
    }

    pub fn make_name(&mut self, cat: &str) -> String {
        self.namectr += 1;
        format!("_asmpp_{}{}", cat, self.namectr)
    }

    pub fn func_prologue(&self, name: &str) -> String {
        if self.pascal {
            [
                format!("procedure {}();", name),
                "type".to_string(),
                " pi = ^integer;".to_string(),
                " pf = ^single;".to_string(),
                " pd = ^double;".to_string(),
                "var".to_string(),
                " vi: pi;".to_string(),
                " vf: pf;".to_string(),
                " vd: pd;".to_string(),
                "begin".to_string(),
                " vi := vi;".to_string(),
                " vf := vf;".to_string(),
                " vd := vd;".to_string(),
            ].join("\n")
        } else {
            format!("void {}(void) {{", name)
        }
    }

    pub fn func_epilogue(&self) -> &'static str {
        if self.pascal {
            "end;"
        } else {
            "}"
        }
    }

    pub fn pascal_assignment(&mut self, tp: &str, val: &str) -> String {
        self.valuectr += 1;
        let address = (8 * self.valuectr) & 0x7FFF;
        format!("v{} := p{}({}); v{}^ := {};", tp, tp, address, tp, val)
    }
}

// TODO: Add tests for all functionality
// TODO: Consider using a builder pattern for GlobalState construction
// TODO: Consider making internal state private and providing accessor methods
// TODO: Consider using custom types for optimization levels instead of strings
