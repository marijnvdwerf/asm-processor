#[derive(Debug, Clone)]
pub struct GlobalState {
    pub min_instr_count: usize,
    pub skip_instr_count: usize,
    pub use_jtbl_for_rodata: bool,
    pub prelude_if_late_rodata: usize,
    pub mips1: bool,
    pub pascal: bool,
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
}
