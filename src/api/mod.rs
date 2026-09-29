//! API as exposed to AI tools and command line.

mod function;
mod memory;

pub use function::{FunctionUpdate, Var, plain_text};
pub use memory::read_memory;
