use asm_processor::{Result, Error};
use clap::Parser;
use std::{
    env,
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(about = "Pre-process .c files and post-process .o files to enable embedding assembly into C")]
struct Args {
    #[arg(help = "Path to compiler (IDO 5.3/7.1)")]
    compiler: String,

    #[arg(help = "Assembler command and flags")]
    assembler: Vec<String>,

    #[arg(help = "Compiler flags and input/output files")]
    compiler_args: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    // Extract output file and input file from compiler args
    let (out_file, in_file, compile_args) = extract_io_files(&args.compiler_args)?;
    
    // Get optimization flags
    let opt_flags: Vec<_> = compile_args.iter()
        .filter(|&arg| matches!(arg.as_str(), "-g3" | "-g" | "-O0" | "-O1" | "-O2" | "-framepointer" | "-KPIC"))
        .cloned()
        .collect();

    // Add -mips1 if -mips2 not present
    let mut asmproc_flags = opt_flags.clone();
    if !compile_args.iter().any(|arg| arg == "-mips2") {
        asmproc_flags.push("-mips1".to_string());
    }
    
    // Create temporary directory
    let tmp_dir = TempDir::new()?;
    let preprocessed_path = tmp_dir.path().join(format!(
        "preprocessed_{}.{}",
        Uuid::new_v4(),
        in_file.extension().and_then(|e| e.to_str()).unwrap_or("c")
    ));

    // Run first pass - preprocess
    let mut asmproc_args = vec![
        in_file.to_string_lossy().to_string()
    ];
    asmproc_args.extend(asmproc_flags);

    let (functions, deps) = {
        let mut out_file = BufWriter::new(File::create(&preprocessed_path)?);
        asm_processor::run(&asmproc_args, Some(&mut out_file), None)?
    };

    // Run compiler
    let mut compile_cmd = vec![args.compiler];
    compile_cmd.extend(compile_args);
    compile_cmd.extend(vec![
        "-I".to_string(),
        in_file.parent().unwrap().to_string_lossy().to_string(),
        "-o".to_string(),
        out_file.to_string_lossy().to_string(),
        preprocessed_path.to_string_lossy().to_string(),
    ]);

    let status = Command::new(&compile_cmd[0])
        .args(&compile_cmd[1..])
        .status()
        .map_err(|e| Error::Io(e))?;

    if !status.success() {
        return Err(Error::InvalidInput(format!(
            "Failed to compile file {}. Command line:\n{}",
            in_file.display(),
            compile_cmd.join(" ")
        )));
    }

    // Run second pass - post-process
    let prelude_path = env::current_dir()?.join("prelude.inc");
    let assembler_str = args.assembler.join(" ");

    let mut post_args = asmproc_args.clone();
    post_args.extend(vec![
        "--post-process".to_string(),
        out_file.to_string_lossy().to_string(),
        "--assembler".to_string(),
        assembler_str,
    ]);

    if prelude_path.exists() {
        post_args.extend(vec![
            "--asm-prelude".to_string(),
            prelude_path.to_string_lossy().to_string(),
        ]);
    }

    asm_processor::run(&post_args, None, Some(functions))?;

    // Write dependency file if we have dependencies
    if !deps.is_empty() {
        let deps_file = out_file.with_extension("asmproc.d");
        let mut f = File::create(deps_file)?;
        writeln!(f, "{}: {}", out_file.display(), deps.join(" \\\n    "))?;
        for dep in deps {
            writeln!(f, "\n{}:", dep)?;
        }
    } else {
        // Try to remove old dependency file if it exists
        let _ = fs::remove_file(out_file.with_extension("asmproc.d"));
    }

    Ok(())
}

fn extract_io_files(args: &[String]) -> Result<(PathBuf, PathBuf, Vec<String>)> {
    let mut compile_args = args.to_vec();
    
    // Find and remove output file
    let out_ind = compile_args.iter()
        .position(|arg| arg == "-o")
        .ok_or_else(|| Error::InvalidInput("Missing output file (-o) argument".into()))?;
    
    if out_ind + 1 >= compile_args.len() {
        return Err(Error::InvalidInput("Missing output file path after -o".into()));
    }
    
    let out_file = PathBuf::from(&compile_args[out_ind + 1]);
    compile_args.remove(out_ind + 1);
    compile_args.remove(out_ind);

    // Last argument should be input file
    let in_file = PathBuf::from(compile_args.pop()
        .ok_or_else(|| Error::InvalidInput("Missing input file argument".into()))?);

    Ok((out_file, in_file, compile_args))
}
