pub mod asm;
pub mod elf;
pub mod utils;
pub mod processor;

pub use utils::{Error, Result};

// Re-export key types that will be commonly used
pub use utils::state::GlobalState;
pub use processor::parse_source;
