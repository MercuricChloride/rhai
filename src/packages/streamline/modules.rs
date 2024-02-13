use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::{plugin::*, Array};
use core::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ModuleKind {
    Map,
    Store,
}

#[derive(Serialize, Clone)]
#[serde(untagged)]
pub enum ModuleInput {
    Map{ map: String},
    Store { store: String, mode: String },
}

impl<'de> Deserialize<'de> for ModuleInput {
    fn deserialize<D>(deserializer: D) -> Result<ModuleInput, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value.get("kind") {
            Some(kind) => {
                match kind {
                    serde_json::Value::String(string) => {
                        match string.as_str() {
                            "map" => {
                                // {kind: "map", input: "map_events"}
                                let input = value["name"].as_str().unwrap().to_string();
                                Ok(ModuleInput::Map { map: input })
                            },
                            "store" => {
                                let store = value["name"].as_str().unwrap().to_string();
                                let mode = value["mode"].as_str().unwrap_or("get").to_string();
                                Ok(ModuleInput::Store { store, mode })
                            }
                            _ => panic!("Unknown module kind")
                        }
                    },
                    _ => panic!("Unknown module kind")
                }
            }
            None => panic!("No module kind specified"),
        }
    }
}

impl ModuleInput {
    pub fn map(map: String) -> Self {
        Self::Map { map }
    }

    pub fn store(store: String, mode: String) -> Self {
        Self::Store { store, mode }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ModuleOutput {
    #[serde(rename = "type")]
    kind: String,
}

impl Default for ModuleOutput {
    fn default() -> Self {
        Self {
            kind: "proto:google.wkt.struct".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePolicy {
    Set,
    SetIfNotExists,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ModuleData {
    name: String,
    kind: ModuleKind,
    inputs: Vec<ModuleInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<ModuleOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    update_policy: Option<UpdatePolicy>,
}

impl ModuleData {
    pub fn new_mfn(name: String, inputs: Vec<ModuleInput>) -> Self {
        Self {
            name,
            kind: ModuleKind::Map,
            inputs,
            output: Some(ModuleOutput::default()),
            update_policy: None,
        }
    }

    pub fn new_sfn(name: String, inputs: Vec<ModuleInput>) -> Self {
        Self {
            name,
            kind: ModuleKind::Store,
            inputs,
            output: None,
            update_policy: None,
        }
    }
}

#[derive(Default, Clone)]
pub struct ModuleDag {
    pub modules: BTreeMap<String, ModuleData>,
}

impl ModuleDag {
    pub fn new() -> Self {
        let mut module_map = BTreeMap::new();
        module_map.insert(
            "map_events".to_string(),
            ModuleData {
                name: "map_events".to_string(),
                kind: ModuleKind::Map,
                inputs: vec![ModuleInput::Map {
                    map: "source".to_string(),
                }],
                output: Some(ModuleOutput {
                    kind: "proto:google.wkt.struct".to_string(),
                }),
                update_policy: None,
            },
        );
        Self {
            modules: module_map,
        }
    }

    pub fn new_shared() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::new()))
    }

    pub fn add_mfn(&mut self, name: String, inputs: Vec<ModuleInput>) {
        self.modules.insert(name.clone(), ModuleData::new_mfn(name, inputs));
    }

    pub fn add_sfn(&mut self, name: String, inputs: Vec<ModuleInput>) {
        self.modules.insert(name.clone(), ModuleData::new_sfn(name, inputs));
    }

    pub fn get_module(&self, name: &str) -> Option<&ModuleData> {
        self.modules.get(name)
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
            let as_json = serde_json::to_string(&module).unwrap();
            let as_dynamic = serde_json::from_str(&as_json).unwrap();
            as_dynamic
        } else {
            "".into()
        }
    }

    // Add a module to the dependency graph.
    //#[rhai_fn(pure)]
    //pub fn add_module(modules: &mut Modules, module: String, dependencies: Array) {
    //    modules.borrow_mut().add_module(module, dependencies.into());
    //}
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
            output: Some(ModuleOutput {
                kind: "proto:google.wkt.struct".to_string(),
            }),
            update_policy: None,
        };
        let as_json = serde_yaml::to_string(&data).unwrap();
        println!("{as_json}");
    }
}
