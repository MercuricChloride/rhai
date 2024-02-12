use std::rc::Rc;

use crate::{plugin::*, Array};
use core::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct ModuleDag {
    pub modules: BTreeMap<String, Array>,
}
impl ModuleDag {
    pub fn new() -> Self {
        Self {
            modules: BTreeMap::new(),
        }
    }

    pub fn new_shared() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::new()))
    }

    pub fn add_module(&mut self, module: String, dependencies: Array) {
        self.modules.insert(module, dependencies);
    }

    pub fn remove_module(&mut self, module: String) {
        self.modules.remove(&module);
    }

    pub fn get_module(&self, module: &str) -> Option<&Array> {
        self.modules.get(module)
    }
}

pub type GlobalModuleDag = Rc<RefCell<ModuleDag>>;

/// The `Modules` module provides functionality for managing the module dependency graph.
#[export_module]
pub mod module_api {
    use crate::Array;

    /// The `Modules` module provides functionality for managing the module dependency graph.
    pub type Modules = GlobalModuleDag;

    /// Get the module dependency graph.
    #[rhai_fn(get = "modules", pure)]
    pub fn get_modules(modules: &mut Modules) -> Dynamic {
        modules.borrow().modules.clone().into()
    }

    /// Get the name of a module's module
    #[rhai_fn(pure)]
    pub fn get(modules: &mut Modules, name: &str) -> Dynamic {
        if let Some(module) = modules.borrow().get_module(name).cloned() {
            module.into()
        } else {
            "".into()
        }
    }

    /// Add a module to the dependency graph.
    #[rhai_fn(pure)]
    pub fn add_module(modules: &mut Modules, module: String, dependencies: Array) {
        modules.borrow_mut().add_module(module, dependencies.into());
    }
}
