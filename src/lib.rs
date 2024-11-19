pub mod elf;
pub mod objfile;
pub mod utils;
pub mod processor;

use std::io::Write;
use std::path::Path;
use clap::{Arg, Command, ArgAction, ArgGroup, value_parser};
use crate::objfile::{fixup_objfile, AsmFunction};
use crate::processor::parse_source;
use crate::utils::{Error, Opts};

/// Main entry point for the asm-processor
///
/// # Arguments
/// * `args` - Command line arguments
/// * `outfile` - Optional writer for preprocessed output
/// * `functions` - Optional list of pre-parsed functions for post-processing
///
/// # Returns
/// * `Result<(Vec<Function>, Vec<String>), Error>` - Tuple of processed functions and dependencies
pub fn run(
    argv: &[String],
    outfile: Option<&mut dyn Write>,
    functions: Option<Vec<Function>>,
) -> Result<(Vec<Function>, Vec<String>), Error> {
    let args = parse_args(argv)?;
    let opts = create_opts(&args)?;

    if args.objfile.is_none() {
        // Pre-processing mode
        let mut deps = Vec::new();
        let file = std::fs::File::open(&args.filename)
            .map_err(|e| Error::Io(e))?;
        let reader = std::io::BufReader::new(file);
        let functions = parse_source(reader, &opts, Some(&mut deps), outfile)?;
        Ok((functions, deps))
    } else {
        // Post-processing mode
        if args.assembler.is_none() {
            return Err(Error::InvalidInput("must pass assembler command".into()));
        }

        let functions = if let Some(funcs) = functions {
            funcs
        } else {
            let file = std::fs::File::open(&args.filename)
                .map_err(|e| Error::Io(e))?;
            let reader = std::io::BufReader::new(file);
            parse_source(reader, &opts, None, None)?
        };

        if functions.is_empty() && !args.force {
            return Ok((Vec::new(), Vec::new()));
        }

        let mut asm_prelude = Vec::new();
        if let Some(prelude_path) = args.asm_prelude {
            let mut file = std::fs::File::open(prelude_path)
                .map_err(|e| Error::Io(e))?;
            file.read_to_end(&mut asm_prelude)
                .map_err(|e| Error::Io(e))?;
        }

        fixup_objfile(
            args.objfile.unwrap(),
            &functions,
            &asm_prelude,
            args.assembler.unwrap(),
            args.output_enc,
            args.drop_mdebug_gptab,
            args.convert_statics,
        )?;

        Ok((Vec::new(), Vec::new()))
    }
}

/// Parse command line arguments
fn parse_args(argv: &[String]) -> Result<Args, Error> {
    let matches = Command::new("asm-processor")
        .about("Pre-process .c files and post-process .o files to enable embedding assembly into C.")
        .arg(Arg::new("filename")
            .help("path to .c code")
            .required(true))
        .arg(Arg::new("post-process")
            .long("post-process")
            .help("path to .o file to post-process")
            .value_name("objfile"))
        .arg(Arg::new("assembler")
            .long("assembler")
            .help("assembler command (e.g. \"mips-linux-gnu-as -march=vr4300 -mabi=32\")"))
        .arg(Arg::new("asm-prelude")
            .long("asm-prelude")
            .help("path to a file containing a prelude to the assembly file (with .set and .macro directives, e.g.)"))
        .arg(Arg::new("input-enc")
            .long("input-enc")
            .help("input encoding")
            .default_value("latin1"))
        .arg(Arg::new("output-enc")
            .long("output-enc")
            .help("output encoding")
            .default_value("latin1"))
        .arg(Arg::new("drop-mdebug-gptab")
            .long("drop-mdebug-gptab")
            .help("drop mdebug and gptab sections")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("convert-statics")
            .long("convert-statics")
            .help("change static symbol visibility")
            .value_parser(["no", "local", "global", "global-with-filename"])
            .default_value("local"))
        .arg(Arg::new("force")
            .long("force")
            .help("force processing of files without GLOBAL_ASM blocks")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("encode-cutscene-data-floats")
            .long("encode-cutscene-data-floats")
            .help("Replace floats with their encoded hexadecimal representation in CutsceneData data")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("framepointer")
            .long("framepointer")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("mips1")
            .long("mips1")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("g3")
            .long("g3")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("KPIC")
            .long("KPIC")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("O0")
            .long("O0")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("O1")
            .long("O1")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("O2")
            .long("O2")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("g")
            .long("g")
            .action(ArgAction::SetTrue))
        .group(ArgGroup::new("opt")
            .args(["O0", "O1", "O2", "g"])
            .required(true))
        .try_get_matches_from(argv)
        .map_err(|e| Error::InvalidInput(e.to_string()))?;

    let opt = if matches.get_flag("O0") {
        "O0"
    } else if matches.get_flag("O1") {
        "O1"
    } else if matches.get_flag("O2") {
        "O2"
    } else {
        "g"
    }.to_string();

    let filename = matches.get_one::<String>("filename")
        .ok_or_else(|| Error::InvalidInput("filename is required".into()))?
        .clone();

    // Check Pascal extension
    let pascal = filename.ends_with(".p") || filename.ends_with(".pas") || filename.ends_with(".pp");

    // Validate g3 flag
    if matches.get_flag("g3") {
        if opt != "O2" {
            return Err(Error::InvalidInput("-g3 is only supported together with -O2".into()));
        }
    }

    // Validate mips1 flag
    if matches.get_flag("mips1") && (opt != "O1" && opt != "O2" || matches.get_flag("framepointer")) {
        return Err(Error::InvalidInput("-mips1 is only supported together with -O1 or -O2".into()));
    }

    // Validate Pascal options
    if pascal && opt != "O1" && opt != "O2" && !(opt == "O2" && matches.get_flag("g3")) {
        return Err(Error::InvalidInput("Pascal is only supported together with -O1, -O2 or -O2 -g3".into()));
    }

    Ok(Args {
        filename,
        objfile: matches.get_one::<String>("post-process").cloned(),
        assembler: matches.get_one::<String>("assembler").cloned(),
        asm_prelude: matches.get_one::<String>("asm-prelude").cloned(),
        input_enc: matches.get_one::<String>("input-enc").unwrap().clone(),
        output_enc: matches.get_one::<String>("output-enc").unwrap().clone(),
        drop_mdebug_gptab: matches.get_flag("drop-mdebug-gptab"),
        convert_statics: matches.get_one::<String>("convert-statics").unwrap().clone(),
        force: matches.get_flag("force"),
        enable_cutscene_data_float_encoding: matches.get_flag("encode-cutscene-data-floats"),
        framepointer: matches.get_flag("framepointer"),
        mips1: matches.get_flag("mips1"),
        g3: matches.get_flag("g3"),
        kpic: matches.get_flag("KPIC"),
        opt,
    })
}

/// Create options from parsed arguments
fn create_opts(args: &Args) -> Result<Opts, Error> {
    Ok(Opts::new(
        &args.opt,
        args.framepointer,
        args.mips1,
        args.kpic,
        args.filename.ends_with(".p") || args.filename.ends_with(".pas") || args.filename.ends_with(".pp"),
        &args.input_enc,
        &args.output_enc,
        args.enable_cutscene_data_float_encoding,
    ))
}

/// Command line arguments structure
#[derive(Debug)]
struct Args {
    filename: String,
    objfile: Option<String>,
    assembler: Option<String>,
    asm_prelude: Option<String>,
    input_enc: String,
    output_enc: String,
    drop_mdebug_gptab: bool,
    convert_statics: String,
    force: bool,
    enable_cutscene_data_float_encoding: bool,
    framepointer: bool,
    mips1: bool,
    g3: bool,
    kpic: bool,
    opt: String,
}
