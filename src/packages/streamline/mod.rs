use super::{Package, StandardPackage};
use crate::{def_package, Array, Engine, Scope};
use rhai_codegen::combine_with_exported_module;

mod abi;
mod blocks;
mod codegen;
/// A plugin to handle the dag of substreams modules
mod modules;
// A plugin to add custom syntax for streamline
//mod syntax;

def_package! {
    /// Streamline package for the substreams module
    pub StreamlinePackage(module): StandardPackage {
        combine_with_exported_module!(module, "module_helpers", modules::module_api);
        combine_with_exported_module!(module, "abi_helpers", abi::abi_api);
        combine_with_exported_module!(module, "block_helpers", blocks::blocks)
        //combine_with_exported_module!(module, "stream_helpers", stream::blocks);
    }
}

/// Initialize the package and scope for the substreams package
pub fn init_package(mut engine: Engine, mut scope: Scope) -> (Engine, Scope) {
    let package = StreamlinePackage::new();
    package.register_into_engine(&mut engine);
    init_globals(&mut engine, &mut scope);
    (engine, scope)
}

/// Initialize the global variables for the substreams package
fn init_globals(engine: &mut Engine, scope: &mut Scope) {
    modules::init_globals(engine, scope);
    #[cfg(not(feature = "substreams_runtime"))]
    abi::init_globals(engine, scope);
    blocks::init_globals(engine, scope);
}
