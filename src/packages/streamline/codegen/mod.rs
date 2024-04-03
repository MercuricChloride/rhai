/// The trait for handling code generation of something
pub trait Codegen {
    /// Generates the code for Self
    fn generate(&self) -> String;
}

/// Code generation for the rust module handlers
pub mod rust;
/// Code generation for the substreams yaml file
pub mod yaml;
