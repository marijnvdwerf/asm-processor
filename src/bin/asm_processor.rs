use std::io::{self, Write};
use std::path::PathBuf;
use clap::Parser;
use asm_processor::run;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
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

fn main() {
    let args = Args::parse();
    
    // Convert Args to Vec<String> for library function
    let mut argv = Vec::new();
    argv.push(args.filename.to_string_lossy().into_owned());
    
    if let Some(post_process) = args.post_process {
        argv.push("--post-process".into());
        argv.push(post_process.to_string_lossy().into_owned());
    }
    
    if let Some(assembler) = args.assembler {
        argv.push("--assembler".into());
        argv.push(assembler);
    }
    
    if let Some(asm_prelude) = args.asm_prelude {
        argv.push("--asm-prelude".into());
        argv.push(asm_prelude.to_string_lossy().into_owned());
    }
    
    argv.push("--input-enc".into());
    argv.push(args.input_enc);
    argv.push("--output-enc".into());
    argv.push(args.output_enc);
    
    if args.drop_mdebug_gptab {
        argv.push("--drop-mdebug-gptab".into());
    }
    
    argv.push("--convert-statics".into());
    argv.push(args.convert_statics);
    
    if args.force {
        argv.push("--force".into());
    }
    
    if args.encode_cutscene_data_floats {
        argv.push("--encode-cutscene-data-floats".into());
    }
    
    if args.framepointer {
        argv.push("--framepointer".into());
    }
    
    if args.mips1 {
        argv.push("--mips1".into());
    }
    
    if args.g3 {
        argv.push("--g3".into());
    }
    
    if args.kpic {
        argv.push("--KPIC".into());
    }
    
    if args.opt_o0 {
        argv.push("--O0".into());
    } else if args.opt_o1 {
        argv.push("--O1".into());
    } else if args.opt_o2 {
        argv.push("--O2".into());
    } else {
        argv.push("-g".into());
    }
    
    match run(&argv, Some(&mut io::stdout()), None) {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
