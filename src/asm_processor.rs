use std::path::{PathBuf, Path};
use std::io::BufReader;
use std::fs::File;
use clap::{Parser, ArgGroup};

use crate::utils::options::Opts;
use crate::asm::Function;
use crate::processor::parse_source;
use crate::objfile::fixup_objfile;
use crate::utils::{Error, Result};

/// Processor output
#[derive(Debug)]
pub struct ProcessorOutput {
    pub functions: Vec<Function>,
    pub dependencies: Vec<String>,
}

#[derive(Parser)]
#[command(author, version, about = "Pre-process .c files and post-process .o files to enable embedding assembly into C.")]
#[command(group(
    ArgGroup::new("optimization")
        .required(true)
        .args(["opt_o0", "opt_o1", "opt_o2", "opt_g"]),
))]
pub struct Args {
    /// Path to .c code
    #[arg(value_name = "FILE")]
    pub filename: PathBuf,

    /// Path to .o file to post-process
    #[arg(long)]
    pub post_process: Option<PathBuf>,

    /// Assembler command (e.g. "mips-linux-gnu-as -march=vr4300 -mabi=32")
    #[arg(long)]
    pub assembler: Option<String>,

    /// Path to a file containing a prelude to the assembly file, or the prelude content itself
    #[arg(long)]
    pub asm_prelude: Option<String>,

    /// Input encoding (default: latin1)
    #[arg(long, default_value = "latin1")]
    pub input_enc: String,

    /// Output encoding (default: latin1)
    #[arg(long, default_value = "latin1")]
    pub output_enc: String,

    /// Drop mdebug and gptab sections
    #[arg(long)]
    pub drop_mdebug_gptab: bool,

    /// Change static symbol visibility
    #[arg(long, value_name = "MODE", default_value = "local")]
    #[arg(value_parser = ["no", "local", "global", "global-with-filename"])]
    pub convert_statics: String,

    /// Force processing of files without GLOBAL_ASM blocks
    #[arg(long)]
    pub force: bool,

    /// Replace floats with their encoded hexadecimal representation in CutsceneData data
    #[arg(long)]
    pub encode_cutscene_data_floats: bool,

    /// Use frame pointer
    #[arg(long)]
    pub framepointer: bool,

    /// Use MIPS1 instructions
    #[arg(long)]
    pub mips1: bool,

    /// Use -g3 debug info
    #[arg(long)]
    pub g3: bool,

    /// Use KPIC
    #[arg(long = "KPIC")]
    pub kpic: bool,

    #[arg(long = "O0")]
    pub opt_o0: bool,

    #[arg(long = "O1")]
    pub opt_o1: bool,

    #[arg(long = "O2")]
    pub opt_o2: bool,

    #[arg(short = 'g')]
    pub opt_g: bool,
}

/// Run the asm-processor with the given arguments
pub fn run<W: std::io::Write>(
    args: &Args, 
    outfile: Option<&mut W>,
    input_functions: Option<Vec<Function>>,
) -> Result<Option<ProcessorOutput>> {
    let opt = match (args.opt_o0, args.opt_o1, args.opt_o2, args.opt_g) {
        (true, _, _, _) => "O0",
        (_, true, _, _) => "O1",
        (_, _, true, _) => "O2",
        (_, _, _, true) => "g",
        _ => unreachable!("clap ensures one option is selected"),
    };

    let pascal = args.filename
        .extension()
        .and_then(|ext| ext.to_str())
        .map_or(false, |ext| matches!(ext, "p" | "pas" | "pp"));

    // Validation checks
    if args.g3 && opt != "O2" {
        return Err(Error::InvalidInput("-g3 is only supported together with -O2".into()));
    }

    let opt = if args.g3 { "g3" } else { opt };

    if args.mips1 && (!matches!(opt, "O1" | "O2") || args.framepointer) {
        return Err(Error::InvalidInput("-mips1 is only supported together with -O1 or -O2".into()));
    }

    if pascal && !matches!(opt, "O1" | "O2" | "g3") {
        return Err(Error::InvalidInput(
            "Pascal is only supported together with -O1, -O2 or -O2 -g3".into()
        ));
    }

    let opts = Opts::new(
        opt,
        args.framepointer,
        args.kpic,
        args.encode_cutscene_data_floats,
        args.mips1,
        pascal,
        &args.filename,
        &args.input_enc,
        &args.output_enc,
    );

    // Handle the case where we're not post-processing first
    if args.post_process.is_none() {
        let mut deps = Vec::new();
        let file = File::open(&args.filename)?;
        let mut reader = BufReader::new(file);
        let functions = parse_source(&mut reader, &opts, &mut deps, outfile)?;
        
        return Ok(Some(ProcessorOutput {
            functions,
            dependencies: deps,
        }));
    }

    // Handle post-processing case
    let objfile = args.post_process.as_ref().unwrap();
    let assembler = args.assembler.as_ref()
        .ok_or_else(|| Error::InvalidInput("must pass assembler command".into()))?;

    let mut deps = Vec::new();
    let functions = if let Some(funcs) = input_functions {
        funcs
    } else {
        let file = File::open(&args.filename)?;
        let mut reader = BufReader::new(file);
        parse_source(&mut reader, &opts, &mut deps, outfile)?
    };

    if functions.is_empty() && !args.force {
        return Ok(None);
    }

    let asm_prelude = match &args.asm_prelude {
        Some(prelude) => {
            if Path::new(prelude).exists() {
                std::fs::read(prelude)?
            } else {
                prelude.as_bytes().to_vec()
            }
        }
        None => Vec::new(),
    };

    fixup_objfile(
        objfile,
        &functions,
        &asm_prelude,
        assembler,
        &args.output_enc,
        args.drop_mdebug_gptab,
        &args.convert_statics,
    )?;

    Ok(None)
}
