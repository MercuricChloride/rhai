use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::{plugin::*, Array};
use core::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ModuleKind {
    Map,
    Store,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum ModuleInput {
    Map { map: String },
    Store { store: String, mode: String },
}

#[derive(Serialize, Deserialize)]
struct ModuleOutput {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Serialize, Deserialize)]
struct ModuleData {
    name: String,
    kind: ModuleKind,
    inputs: Vec<ModuleInput>,
    output: ModuleOutput,
    #[serde(skip_serializing_if = "Option::is_none")]
    update_policy: Option<String>,
}

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

mod test {
    use super::*;

    #[test]
    fn test_serialize() {
        let data = ModuleData {
            name: "test".to_string(),
            kind: ModuleKind::Map,
            inputs: vec![ModuleInput::Map {
                map: "map_events".to_string(),
            }],
            output: ModuleOutput {
                kind: "proto:google.wkt.struct".to_string(),
            },
            update_policy: Some("set_if_not_exists".to_string()),
        };
        let as_json = serde_yaml::to_string(&data).unwrap();
        println!("{as_json}");
    }
}
