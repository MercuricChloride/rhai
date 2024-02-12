use rhai_codegen::{combine_with_exported_module, exported_module};

use crate::{def_package, Engine, Scope};

use super::{Package, StandardPackage};

/// A plugin to handle the dag of substreams modules
mod modules;

def_package! {
    /// Streamline package for the substreams module
    pub StreamlinePackage(module): StandardPackage {
        combine_with_exported_module!(module, "module_helpers", modules::module_api);
    }
}

/// Initialize the package and scope for the substreams package
pub fn init_package(mut engine: Engine, mut scope: Scope) -> (Engine, Scope) {
    let package = StreamlinePackage::new();
    package.register_into_engine(&mut engine);
    init_globals(&mut scope);
    (engine, scope)
}

/// Initialize the global variables for the substreams package
fn init_globals(scope: &mut Scope) {
    let module_dag = modules::ModuleDag::new_shared();

    scope.push_constant("MODULES", module_dag);
}
