#![recursion_limit = "512"]
uniffi::setup_scaffolding!();

mod email;
pub use email::*;

mod tokenizers;
pub use tokenizers::*;
