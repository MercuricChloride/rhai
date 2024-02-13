use rhai_codegen::{combine_with_exported_module, exported_module};

use crate::{def_package, Array, Engine, Scope};

use self::modules::ModuleDag;

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
    init_globals(&mut engine, &mut scope);
    (engine, scope)
}

macro_rules! convert {
    ($value: expr) => {
        serde_json::from_str(&serde_json::to_string(&$value).unwrap()).unwrap()
    };
}

/// Initialize the global variables for the substreams package
fn init_globals(engine: &mut Engine, scope: &mut Scope) {
    let module_dag = modules::ModuleDag::new_shared();

    let modules = module_dag.clone();
    // TODO - change this to accept in an array of strings, which we will look up to resolve input types
    engine.register_fn("add_mfn", 
    move |name: String, inputs: Array| {
        (*modules).borrow_mut().add_mfn(name, convert!(inputs));
    });

    let modules = module_dag.clone();
    engine.register_fn("add_sfn", 
    move |name: String, inputs: Array| {
        (*modules).borrow_mut().add_sfn(name, convert!(inputs));
    });
    
    scope.push_constant("MODULES", module_dag);
}
