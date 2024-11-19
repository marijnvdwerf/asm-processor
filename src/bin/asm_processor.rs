use asm_processor::{Args, run, Result};
use clap::Parser;

fn main() -> Result<()> {
    let args = Args::parse();
    if let Err(e) = run(&args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    Ok(())
}
