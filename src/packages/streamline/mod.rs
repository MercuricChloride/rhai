use rhai_codegen::{combine_with_exported_module, exported_module};

use crate::{def_package, Array, Engine, Scope};

use super::{Package, StandardPackage};

/// A plugin to handle the dag of substreams modules
mod modules;
mod abi;


def_package! {
    /// Streamline package for the substreams module
    pub StreamlinePackage(module): StandardPackage {
        combine_with_exported_module!(module, "module_helpers", modules::module_api);
        combine_with_exported_module!(module, "contract_source_helpers", abi::abi_api);
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
pub fn init_globals(engine: &mut Engine, scope: &mut Scope) {
    modules::init_globals(engine, scope);
    abi::init_globals(engine, scope);
}

/// A macro to convert a type, into another that shares the same serialization
/// IE Type A -> JSON -> Type B
#[macro_export]
macro_rules! convert {
    ($value: expr) => {
        serde_json::from_str(&serde_json::to_string(&$value).unwrap()).unwrap()
    };
}
