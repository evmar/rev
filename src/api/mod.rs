//! API as exposed to AI tools and command line.

mod func;
mod memory;

pub use func::{FunctionUpdate, Var, plain_text};
pub use memory::read_memory;
