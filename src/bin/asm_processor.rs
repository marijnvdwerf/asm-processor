use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use clap::{Parser, ArgGroup};

use asm_processor::{
    Error,
    Result,
    Function,
    parse_source,
    utils::options::Opts,
    objfile::fixup_objfile,
};

#[derive(Parser)]
#[command(author, version, about = "Pre-process .c files and post-process .o files to enable embedding assembly into C.")]
#[command(group(
    ArgGroup::new("optimization")
        .required(true)
        .args(["opt_o0", "opt_o1", "opt_o2", "opt_g"]),
))]
struct Args {
    /// Path to .c code
    #[arg(value_name = "FILE")]
    filename: PathBuf,

    /// Path to .o file to post-process
    #[arg(long)]
    post_process: Option<PathBuf>,

    /// Assembler command (e.g. "mips-linux-gnu-as -march=vr4300 -mabi=32")
    #[arg(long)]
    assembler: Option<String>,

    /// Path to a file containing a prelude to the assembly file
    #[arg(long)]
    asm_prelude: Option<PathBuf>,

    /// Input encoding (default: latin1)
    #[arg(long, default_value = "latin1")]
    input_enc: String,

    /// Output encoding (default: latin1)
    #[arg(long, default_value = "latin1")]
    output_enc: String,

    /// Drop mdebug and gptab sections
    #[arg(long)]
    drop_mdebug_gptab: bool,

    /// Change static symbol visibility
    #[arg(long, value_name = "MODE", default_value = "local")]
    #[arg(value_parser = ["no", "local", "global", "global-with-filename"])]
    convert_statics: String,

    /// Force processing of files without GLOBAL_ASM blocks
    #[arg(long)]
    force: bool,

    /// Replace floats with their encoded hexadecimal representation in CutsceneData data
    #[arg(long)]
    encode_cutscene_data_floats: bool,

    /// Use frame pointer
    #[arg(long)]
    framepointer: bool,

    /// Use MIPS1 instructions
    #[arg(long)]
    mips1: bool,

    /// Use -g3 debug info
    #[arg(long)]
    g3: bool,

    /// Use KPIC
    #[arg(long = "KPIC")]
    kpic: bool,

    #[arg(long = "O0")]
    opt_o0: bool,

    #[arg(long = "O1")]
    opt_o1: bool,

    #[arg(long = "O2")]
    opt_o2: bool,

    #[arg(short = 'g')]
    opt_g: bool,
}

/// Run the asm-processor with the given arguments
///
/// This is the main entry point for the library. It handles both pre-processing
/// source files and post-processing object files.
pub fn run_wrapped(args: Args, outfile: Option<&mut dyn Write>) -> Result<(Vec<Function>, Vec<String>)> {
    let opt = if args.opt_o0 {
        "O0"
    } else if args.opt_o1 {
        "O1"
    } else if args.opt_o2 {
        "O2"
    } else {
        "g"
    }.to_string();

    let pascal = args.filename
        .extension()
        .map(|ext| matches!(ext.to_str(), Some("p" | "pas" | "pp")))
        .unwrap_or(false);

    if args.g3 && opt != "O2" {
        return Err(Error::InvalidInput("-g3 is only supported together with -O2".into()));
    }

    let opt = if args.g3 { "g3".to_string() } else { opt };

    if args.mips1 && (!matches!(opt.as_str(), "O1" | "O2") || args.framepointer) {
        return Err(Error::InvalidInput("-mips1 is only supported together with -O1 or -O2".into()));
    }

    if pascal && !matches!(opt.as_str(), "O1" | "O2" | "g3") {
        return Err(Error::InvalidInput(
            "Pascal is only supported together with -O1, -O2 or -O2 -g3".into()
        ));
    }

    let opts = Opts::new(
        &opt,
        args.framepointer,
        args.mips1,
        args.kpic,
        pascal,
        &args.input_enc,
        &args.output_enc,
        args.encode_cutscene_data_floats,
    );

    if let Some(objfile) = args.post_process {
        if args.assembler.is_none() {
            return Err(Error::InvalidInput("must pass assembler command".into()));
        }

        let functions = {
            let mut file = BufReader::new(File::open(&args.filename)?);
            parse_source(&mut file, &opts, &mut Vec::new(), None)?
        };

        if functions.is_empty() && !args.force {
            return Ok((Vec::new(), Vec::new()));
        }

        let asm_prelude = if let Some(prelude_path) = args.asm_prelude {
            std::fs::read(prelude_path)?
        } else {
            Vec::new()
        };

        fixup_objfile(
            &objfile,
            &functions,
            &asm_prelude,
            args.assembler.as_ref().unwrap(),
            &args.output_enc,
            args.drop_mdebug_gptab,
            &args.convert_statics,
        )?;

        Ok((functions, Vec::new()))
    } else {
        let mut deps = Vec::new();
        let mut file = BufReader::new(File::open(&args.filename)?);
        
        let functions = if let Some(out) = outfile {
            let mut writer = BufWriter::new(out);
            parse_source(&mut file, &opts, &mut deps, Some(&mut writer))?
        } else {
            parse_source(&mut file, &opts, &mut deps, None)?
        };

        Ok((functions, deps))
    }
}

/// Run the asm-processor with command line arguments
///
/// This is the main entry point for the command line interface.
pub fn run(argv: &[String], outfile: Option<&mut dyn Write>, functions: Option<Vec<Function>>) -> Result<(Vec<Function>, Vec<String>)> {
    let args = Args::try_parse_from(argv)
        .map_err(|e| Error::InvalidInput(e.to_string()))?;
    run_wrapped(args, outfile)
}

fn main() {
    match run(&std::env::args().collect::<Vec<_>>(), Some(&mut io::stdout()), None) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
