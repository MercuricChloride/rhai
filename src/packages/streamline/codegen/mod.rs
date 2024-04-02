pub trait Codegen {
    fn generate(&self) -> String;
}

pub mod rust;
pub mod yaml;
