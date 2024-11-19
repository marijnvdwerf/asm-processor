pub mod asm;
pub mod elf;
pub mod utils;
pub mod processor;
pub mod objfile;
pub mod asm_processor;

pub use utils::{Error, Result};
// Re-export key types that will be commonly used
pub use utils::state::GlobalState;
pub use processor::parse_source;
pub use objfile::fixup_objfile;
pub use asm_processor::{run, Args, ProcessorOutput};

use asm::Function;
