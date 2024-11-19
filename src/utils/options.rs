use std::path::PathBuf;

/// Options for controlling the assembly processing
#[derive(Debug, Clone)]
pub struct Opts {
    /// Optimization level (O0, O1, O2, g, g3)
    pub opt: String,
    /// Whether to use frame pointer
    pub framepointer: bool,
    /// Whether to use KPIC (Position Independent Code)
    pub kpic: bool,
    /// Whether to enable cutscene data float encoding
    pub enable_cutscene_data_float_encoding: bool,
    /// Whether targeting MIPS1
    pub mips1: bool,
    /// Whether using Pascal calling convention
    pub pascal: bool,
    /// Input filename
    pub filename: PathBuf,
    /// Output encoding
    pub output_enc: String,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            opt: "O2".to_string(),
            framepointer: false,
            kpic: false,
            enable_cutscene_data_float_encoding: false,
            mips1: false,
            pascal: false,
            filename: PathBuf::from("input.c"),
            output_enc: "utf-8".to_string(),
        }
    }
}

impl Opts {
    /// Create new options with custom settings
    pub fn new(
        opt: impl Into<String>,
        framepointer: bool,
        kpic: bool,
        enable_cutscene_data_float_encoding: bool,
        mips1: bool,
        pascal: bool,
        filename: impl Into<PathBuf>,
        output_enc: impl Into<String>,
    ) -> Self {
        Self {
            opt: opt.into(),
            framepointer,
            kpic,
            enable_cutscene_data_float_encoding,
            mips1,
            pascal,
            filename: filename.into(),
            output_enc: output_enc.into(),
        }
    }
}
