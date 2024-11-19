use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to process assembly: {0}")]
    AssemblyProcessing(String),

    #[error("Failed to process ELF file: {0}")]
    ElfProcessing(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Encoding error: {0}")]
    EncodingError(String),

    #[error("Invalid section: {0}")]
    InvalidSection(String),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),

    #[error("Invalid relocation: {0}")]
    InvalidRelocation(String),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),
}

pub type Result<T> = std::result::Result<T, Error>;
