use std::{
    env,
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
};

use asm_processor::{run, parse_source, utils::options::Opts, ProcessorOutput, Args};
use tempfile::TempDir;
use uuid::Uuid;

// Include the default prelude content at compile time
const DEFAULT_PRELUDE: &str = include_str!("../../prelude.inc");

#[derive(Debug)]
struct BuildConfig {
    asmproc_flags: Vec<String>,
    compiler: Vec<String>,
    assembler_args: Vec<String>,
    compile_args: Vec<String>,
    out_file: PathBuf,
    in_file: PathBuf,
    keep_preprocessed: bool,
}

fn parse_args() -> BuildConfig {
    let args: Vec<String> = env::args().skip(1).collect();
    
    // Find separators
    let sep0 = args.iter()
        .position(|arg| !arg.starts_with('-'))
        .expect("No compiler command found");
    let sep1 = args.iter()
        .position(|arg| arg == "--")
        .expect("No first -- separator found");
    let sep2 = args.iter()
        .skip(sep1 + 1)
        .position(|arg| arg == "--")
        .map(|pos| pos + sep1 + 1)
        .expect("No second -- separator found");

    // Split arguments into their respective groups
    let mut asmproc_flags: Vec<String> = args[..sep0].to_vec();
    let compiler = args[sep0..sep1].to_vec();
    let assembler_args = args[sep1 + 1..sep2].to_vec();
    let mut compile_args = args[sep2 + 1..].to_vec();

    // Extract output file
    let out_ind = compile_args.iter()
        .position(|arg| arg == "-o")
        .expect("No output file specified");
    let out_file = PathBuf::from(&compile_args[out_ind + 1]);
    compile_args.remove(out_ind + 1);
    compile_args.remove(out_ind);

    // Extract input file
    let in_file = PathBuf::from(compile_args.pop().expect("No input file specified"));

    // Get optimization flags
    let opt_flags: Vec<String> = compile_args.iter()
        .filter(|&x| ["-g3", "-g", "-O0", "-O1", "-O2", "-framepointer", "-KPIC"].contains(&x.as_str()))
        .cloned()
        .collect();

    if !compile_args.contains(&"-mips2".to_string()) {
        asmproc_flags.push("-mips1".to_string());
    }

    asmproc_flags.extend(opt_flags);
    asmproc_flags.push(in_file.to_string_lossy().into_owned());

    // Get asm_prelude path
    let dir_path = env::current_exe()
        .expect("Failed to get executable path")
        .parent()
        .expect("Failed to get executable directory")
        .to_path_buf();
    let asm_prelude_path = dir_path.join("prelude.inc");

    BuildConfig {
        asmproc_flags,
        compiler,
        assembler_args,
        compile_args,
        out_file,
        in_file,
        keep_preprocessed: false,
    }
}

fn run_preprocessor(
    config: &BuildConfig,
    temp_dir: &Path,
) -> Result<(Option<ProcessorOutput>, PathBuf), Box<dyn std::error::Error>> {
    let preprocessed_filename = format!(
        "preprocessed_{}.{}",
        Uuid::new_v4(),
        config.in_file.extension().unwrap_or_default().to_string_lossy()
    );
    let preprocessed_path = temp_dir.join(&preprocessed_filename);

    // Create Args struct for first asm_processor run
    let args = Args {
        filename: config.in_file.clone(),
        post_process: None,
        assembler: None,
        asm_prelude: None,
        input_enc: "latin1".to_string(),
        output_enc: "latin1".to_string(),
        drop_mdebug_gptab: false,
        convert_statics: "local".to_string(),
        force: false,
        encode_cutscene_data_floats: false,
        framepointer: config.asmproc_flags.contains(&"-framepointer".to_string()),
        mips1: config.asmproc_flags.contains(&"-mips1".to_string()),
        g3: config.asmproc_flags.contains(&"-g3".to_string()),
        kpic: config.asmproc_flags.contains(&"-KPIC".to_string()),
        opt_o0: config.asmproc_flags.contains(&"-O0".to_string()),
        opt_o1: config.asmproc_flags.contains(&"-O1".to_string()),
        opt_o2: config.asmproc_flags.contains(&"-O2".to_string()),
        opt_g: config.asmproc_flags.contains(&"-g".to_string()),
    };

    // Run first pass of asm_processor
    let output = run(&args)?;

    if config.keep_preprocessed {
        let keep_dir = PathBuf::from("./asm_processor_preprocessed");
        fs::create_dir_all(&keep_dir)?;
        let keep_name = format!(
            "{}_{}", 
            config.in_file.file_stem().unwrap().to_string_lossy(),
            preprocessed_filename
        );
        fs::copy(&preprocessed_path, keep_dir.join(keep_name))?;
    }

    Ok((output, preprocessed_path))
}

fn run_compiler(
    config: &BuildConfig,
    preprocessed_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let in_dir = config.in_file.parent().unwrap_or_else(|| Path::new("."));
    
    let mut compile_command = Command::new(&config.compiler[0]);
    compile_command
        .args(&config.compiler[1..])
        .args(&config.compile_args)
        .arg("-I")
        .arg(in_dir)
        .arg("-o")
        .arg(&config.out_file)
        .arg(preprocessed_path);

    let status = compile_command.status()?;
    if !status.success() {
        return Err(format!(
            "Failed to compile file {}. Command line:\n{}",
            config.in_file.display(),
            compile_command
                .get_args()
                .map(|arg| arg.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        ).into());
    }

    Ok(())
}

fn write_deps_file(
    config: &BuildConfig,
    deps: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let deps_file = config.out_file.with_extension("asmproc.d");
    
    if !deps.is_empty() {
        let mut file = File::create(deps_file)?;
        writeln!(
            file,
            "{}: {}", 
            config.out_file.display(),
            deps.join(" \\\n    ")
        )?;
        
        for dep in deps {
            writeln!(file, "\n{}:", dep)?;
        }
    } else if deps_file.exists() {
        fs::remove_file(deps_file)?;
    }

    Ok(())
}

fn run_post_processor(
    config: &BuildConfig,
    output: &ProcessorOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create Args struct for post-processing
    let args = Args {
        filename: config.in_file.clone(),
        post_process: Some(config.out_file.clone()),
        assembler: Some(config.assembler_args.join(" ")),
        asm_prelude: Some(DEFAULT_PRELUDE.to_string()),
        input_enc: "latin1".to_string(),
        output_enc: "latin1".to_string(),
        drop_mdebug_gptab: false,
        convert_statics: "local".to_string(),
        force: false,
        encode_cutscene_data_floats: false,
        framepointer: config.asmproc_flags.contains(&"-framepointer".to_string()),
        mips1: config.asmproc_flags.contains(&"-mips1".to_string()),
        g3: config.asmproc_flags.contains(&"-g3".to_string()),
        kpic: config.asmproc_flags.contains(&"-KPIC".to_string()),
        opt_o0: config.asmproc_flags.contains(&"-O0".to_string()),
        opt_o1: config.asmproc_flags.contains(&"-O1".to_string()),
        opt_o2: config.asmproc_flags.contains(&"-O2".to_string()),
        opt_g: config.asmproc_flags.contains(&"-g".to_string()),
    };

    run(&args)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = parse_args();
    let temp_dir = TempDir::new()?;
    
    // Run preprocessor
    let (output, preprocessed_path) = run_preprocessor(&config, temp_dir.path())?;
    
    // Run compiler
    run_compiler(&config, &preprocessed_path)?;
    
    // Run post-processor if we have functions to process
    if let Some(output) = output {
        run_post_processor(&config, &output)?;
        write_deps_file(&config, &output.dependencies)?;
    }

    Ok(())
}
