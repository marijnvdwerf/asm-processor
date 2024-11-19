use asm_processor::{Args, run, Result};
use clap::Parser;
use std::io::{self, BufWriter};

fn main() -> Result<()> {
    let args = Args::parse();
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout);
    
    if let Err(e) = run(&args, Some(&mut writer)) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    Ok(())
}
