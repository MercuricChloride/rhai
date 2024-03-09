use serde::{Deserialize, Serialize};

use crate::serde::from_dynamic;
use crate::{plugin::*, tokenizer::Token, Array, Scope};
use core::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::rc::Rc;

use super::codegen;

const JSON_PROTO: &str = "proto:google.protobuf.Struct";

enum AccessMode {
    Get,
    Deltas,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ModuleKind {
    Map,
    Store,
    Source,
}

#[derive(Serialize, Clone)]
#[serde(untagged)]
pub enum ModuleInput {
    Map { map: String },
    Store { store: String, mode: String },
    Source { source: String },
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
                            }
                            "store" => {
                                let store = value["name"].as_str().unwrap().to_string();
                                let mode = value["mode"].as_str().unwrap_or("get").to_string();
                                Ok(ModuleInput::Store { store, mode })
                            }
                            "source" => Ok(ModuleInput::eth_block()),
                            _ => panic!("Unknown module kind"),
                        }
                    }
                    _ => panic!("Unknown module kind"),
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

    pub fn store(store: String, mode: AccessMode) -> Self {
        let mode = match mode {
            AccessMode::Get => "get".to_string(),
            AccessMode::Deltas => "deltas".to_string(),
        };
        Self::Store { store, mode }
    }

    pub fn name(&self) -> String {
        match self {
            ModuleInput::Map { map } => map.to_string(),
            ModuleInput::Store { store, mode: _ } => store.to_string(),
            ModuleInput::Source { source: _ } => "block".to_string(),
        }
    }

    pub fn eth_block() -> Self {
        Self::Source {
            source: "sf.ethereum.type.v2.Block".to_string(),
        }
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
            kind: JSON_PROTO.to_string(),
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
    #[serde(skip_serializing)]
    rhai_handler: String,
    kind: ModuleKind,
    inputs: Vec<ModuleInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<ModuleOutput>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "valueType")]
    value_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "updatePolicy")]
    update_policy: Option<UpdatePolicy>,
}

impl ModuleData {
    pub fn new_mfn(name: String, inputs: Vec<ModuleInput>) -> Self {
        Self {
            rhai_handler: name.clone(),
            name,
            kind: ModuleKind::Map,
            inputs,
            output: Some(ModuleOutput::default()),
            update_policy: None,
            value_type: None,
        }
    }

    pub fn new_sfn(name: String, inputs: Vec<ModuleInput>, update_policy: UpdatePolicy) -> Self {
        Self {
            rhai_handler: name.clone(),
            name,
            kind: ModuleKind::Store,
            inputs,
            output: None,
            update_policy: Some(update_policy),
            value_type: Some(JSON_PROTO.to_string()),
        }
    }

    pub fn eth_block() -> Self {
        Self {
            name: "block".to_string(),
            rhai_handler: "block".to_string(),
            kind: ModuleKind::Source,
            inputs: vec![],
            output: Some(ModuleOutput {
                kind: "sf.ethereum.type.v2.Block".to_string(),
            }),
            update_policy: None,
            value_type: None,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn inputs(&self) -> &Vec<ModuleInput> {
        &self.inputs
    }

    pub fn kind(&self) -> &ModuleKind {
        &self.kind
    }

    pub fn handler(&self) -> &str {
        &self.rhai_handler
    }
}

#[derive(Default, Clone)]
pub struct ModuleDag {
    pub modules: BTreeMap<String, ModuleData>,
}

impl ModuleDag {
    pub fn new() -> Self {
        let mut module_map = BTreeMap::new();

        module_map.insert("BLOCK".to_string(), ModuleData::eth_block());

        // module_map.insert(
        //     "map_events".to_string(),
        //     ModuleData {
        //         name: "map_events".to_string(),
        //         rhai_handler: "map_events".to_string(),
        //         kind: ModuleKind::Map,
        //         inputs: vec![ModuleInput::Map {
        //             map: "source".to_string(),
        //         }],
        //         output: Some(ModuleOutput {
        //             kind: JSON_PROTO.to_string(),
        //         }),
        //         update_policy: None,
        //         value_type: None,
        //     },
        // );
        Self {
            modules: module_map,
        }
    }

    pub fn new_shared() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::new()))
    }

    pub fn add_mfn(&mut self, name: String, inputs: Array) {
        let input_names = inputs
            .into_iter()
            .map(|e| from_dynamic(&e).expect("Should be a list of strings"))
            .collect::<Vec<String>>();

        let inputs = input_names
            .iter()
            .map(|input| {
                // This is used if we have a store that isn't being accessed in the default mode
                let mut access_mode: AccessMode = AccessMode::Get;
                let name: &str;

                match input {
                    s if s.ends_with(":deltas") => {
                        name = input.trim_end_matches(":deltas");
                        access_mode = AccessMode::Deltas;
                    }
                    s if s.ends_with(":get") => {
                        name = input.trim_end_matches(":get");
                    }
                    _ => {
                        name = input;
                    }
                }

                let module = self
                    .get_module(&name)
                    .expect(&format!("No module found with name {:?}", input));
                (module, access_mode)
            })
            .map(|(module, access_mode)| match module.kind() {
                ModuleKind::Map => ModuleInput::map(module.name().to_string()),
                ModuleKind::Store => ModuleInput::store(module.name().to_string(), access_mode),
                ModuleKind::Source => ModuleInput::eth_block(),
            })
            .collect::<Vec<_>>();

        self.modules
            .insert(name.clone(), ModuleData::new_mfn(name, inputs));
    }

    pub fn add_sfn(&mut self, name: String, inputs: Array, update_policy: String) {
        let update_policy = match update_policy.as_str() {
            "set" => UpdatePolicy::Set,
            "setOnce" => UpdatePolicy::SetIfNotExists,
            _ => panic!("Unknown update policy!")
        };

        let input_names = inputs
            .into_iter()
            .map(|e| from_dynamic(&e).expect("Should be a list of strings"))
            .collect::<Vec<String>>();

        let inputs = input_names
            .iter()
            .map(|input| {
                // This is used if we have a store that isn't being accessed in the default mode
                let mut access_mode: AccessMode = AccessMode::Get;
                let name: &str;

                match input {
                    s if s.ends_with(":deltas") => {
                        name = input.trim_end_matches(":deltas");
                        access_mode = AccessMode::Deltas;
                    }
                    s if s.ends_with(":get") => {
                        name = input.trim_end_matches(":get");
                    }
                    _ => {
                        name = input;
                    }
                }

                let module = self
                    .get_module(&name)
                    .expect(&format!("No module found with name {:?}", input));
                (module, access_mode)
            })
            .map(|(module, access_mode)| match module.kind() {
                ModuleKind::Map => ModuleInput::map(module.name().to_string()),
                ModuleKind::Store => ModuleInput::store(module.name().to_string(), access_mode),
                ModuleKind::Source => ModuleInput::eth_block(),
            })
            .collect::<Vec<_>>();

        self.modules
            .insert(name.clone(), ModuleData::new_sfn(name, inputs, update_policy));
    }

    pub fn get_module(&self, name: &str) -> Option<&ModuleData> {
        self.modules.get(name)
    }

    pub fn generate_streamline_modules(&self) -> String {
        let modules = self.modules.values().collect::<Vec<_>>();
        codegen::rust::generate_streamline_modules(&modules)
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

    // #[rhai_fn(pure)]
    // pub fn eval_module(
    //     modules: &mut Modules,
    //     name: &str,
    //     block_data: Dynamic,
    //     engine: &mut Engine,
    // ) -> Dynamic {
    //     // So to eval a module, we need to eval it's inputs, until we get to something that has an input of a source, then we can eval that module with the block data as inputs
    //     if let Some(module) = modules.borrow().get_module(name).cloned() {
    //         let inputs = module
    //             .inputs
    //             .iter()
    //             .map(|input| match input {
    //                 ModuleInput::Map { map } => eval_module(modules, &map, block_data.clone()),
    //                 ModuleInput::Store { store, mode } => {
    //                     if mode == "get" {
    //                         eval_module(modules, &store, block_data.clone())
    //                     } else {
    //                         panic!("Deltas mode not supported yet")
    //                     }
    //                 }
    //                 ModuleInput::Source { source } => {
    //                     if source == "block" {
    //                         block_data.clone()
    //                     } else {
    //                         panic!("Unknown source")
    //                     }
    //                 }
    //             })
    //             .collect::<Vec<_>>();

    //         let handler = module.handler();
    //     } else {
    //         "".into()
    //     }
    // }
}

#[derive(Serialize, Deserialize)]
struct ModuleConfig {
    name: String,
    inputs: Array,
}

pub fn init_globals(engine: &mut Engine, scope: &mut Scope) {
    let module_dag = ModuleDag::new_shared();

    let modules = module_dag.clone();
    // TODO - change this to accept in an array of strings, which we will look up to resolve input types
    engine.register_fn("add_mfn", move |name: Dynamic, inputs: Dynamic| {
        let name = from_dynamic(&name).unwrap();
        let inputs = from_dynamic(&inputs).unwrap();

        (*modules).borrow_mut().add_mfn(name, inputs);
        "Added mfn to DAG!".to_string()
    });

    let modules = module_dag.clone();
    engine.register_fn("add_sfn", move |name: Dynamic, inputs: Dynamic, update_policy: String| {
        let name = from_dynamic(&name).unwrap();
        let inputs = from_dynamic(&inputs).unwrap();
        (*modules).borrow_mut().add_sfn(name, inputs, update_policy);
        "Added sfn to DAG!".to_string()
    });

    let modules = module_dag.clone();
    engine.register_fn("generate_yaml", move |path: String| {
        let modules = (*modules).borrow();

        let yaml = codegen::yaml::generate_yaml(&modules);
        fs::write(&path, &yaml).unwrap();
        format!("Wrote yaml to {} successfully!", &path)
    });

    let modules = module_dag.clone();
    engine.register_fn("generate_rust", move |path: String| {
        let modules_source = (*modules).borrow().generate_streamline_modules();
        fs::write(&path, &modules_source).unwrap();
        format!("Wrote rust source to {} successfully!", &path)
    });

    // we use the substreams_runtime feature only when we are running in the substreams, not in the repl
    engine.register_fn("in_repl", move || {
        if cfg!(feature = "substreams_runtime") {
            false
        } else {
            true
        }
    });

    scope.push_constant("MODULES", module_dag);
}

// engine.on_parse_token(|token, _, _| {
//     match token {
//         // If we have a address literal that is 42 characters long, we want to convert it to an address type
//         Token::Identifier(s) if s.starts_with("0x") && s.len() == 42 => {
//             //let replacement = format!("address({s})");
//             let replacement = format!("print(123457890)");
//             Token::Identifier(Box::new(replacement.into()))
//         }
//         Token::Custom(value) => {
//             panic!("CUSTOM VALUE")
//         }
//         _ => token
//     }
// });

// mod test {
//     use super::*;

//     #[test]
//     fn test_serialize() {
//         let data = ModuleData {
//             name: "test".to_string(),
//             kind: ModuleKind::Map,
//             inputs: vec![ModuleInput::Map {
//                 map: "map_events".to_string(),
//             }],
//             output: Some(ModuleOutput {
//                 kind: "proto:google.wkt.struct".to_string(),
//             }),
//             update_policy: None,
//         };
//         let as_json = serde_yaml::to_string(&data).unwrap();
//         println!("{as_json}");
//     }
// }
