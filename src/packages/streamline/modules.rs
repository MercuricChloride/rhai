use serde::{Deserialize, Serialize};

use crate::serde::from_dynamic;
use crate::{plugin::*, tokenizer::Token, Array, Scope};
use core::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::rc::Rc;

use super::codegen;
use super::sink::{GlobalSinkConfig, SinkConfigMap};

const JSON_VALUE_PROTO: &str = "proto:google.protobuf.Value";
const BIGINT_PROTO: &str = "bigint";
const INITIAL_BLOCK: Option<i64> = Some(72491700);

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
    Map {
        map: String,
    },
    Store {
        store: String,
        mode: String,
        #[serde(skip)]
        value_type: String,
    },
    Source {
        source: String,
    },
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
                                Ok(ModuleInput::Store {
                                    store,
                                    mode,
                                    value_type: String::new(),
                                })
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

    pub fn store(store: String, mode: AccessMode, value_type: String) -> Self {
        let mode = match mode {
            AccessMode::Get => "get".to_string(),
            AccessMode::Deltas => "deltas".to_string(),
        };
        Self::Store {
            store,
            mode,
            value_type,
        }
    }

    pub fn name(&self) -> String {
        match self {
            ModuleInput::Map { map } => map.to_string(),
            ModuleInput::Store { store, .. } => store.to_string(),
            ModuleInput::Source { .. } => "block".to_string(),
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
            kind: JSON_VALUE_PROTO.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePolicy {
    Set,
    SetIfNotExists,
    Add,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ModuleData {
    name: String,
    #[serde(skip_serializing)]
    rhai_handler: String,
    #[serde(skip_serializing)]
    pub is_sink: bool,
    kind: ModuleKind,
    #[serde(skip_serializing_if = "Option::is_none", rename = "initialBlock")]
    initial_block: Option<i64>,
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
            is_sink: false,
            kind: ModuleKind::Map,
            inputs,
            initial_block: INITIAL_BLOCK.into(),
            output: Some(ModuleOutput::default()),
            update_policy: None,
            value_type: None,
        }
    }

    pub fn new_sink(
        name: String,
        inputs: Vec<ModuleInput>,
        output_type: String,
        module_name: String,
    ) -> Self {
        Self {
            rhai_handler: name.clone(),
            name: module_name,
            is_sink: true,
            kind: ModuleKind::Map,
            inputs,
            initial_block: INITIAL_BLOCK.into(),
            output: Some(ModuleOutput { kind: output_type }),
            update_policy: None,
            value_type: None,
        }
    }

    pub fn new_sfn(name: String, inputs: Vec<ModuleInput>, update_policy: UpdatePolicy) -> Self {
        let value_type = match &update_policy {
            UpdatePolicy::Add => Some(BIGINT_PROTO.to_string()),
            _ => Some(JSON_VALUE_PROTO.to_string()),
        };
        Self {
            rhai_handler: name.clone(),
            name,
            is_sink: false,
            kind: ModuleKind::Store,
            inputs,
            initial_block: INITIAL_BLOCK.into(),
            output: None,
            update_policy: Some(update_policy),
            value_type,
        }
    }

    pub fn eth_block() -> Self {
        Self {
            name: "block".to_string(),
            rhai_handler: "block".to_string(),
            kind: ModuleKind::Source,
            is_sink: false,
            initial_block: None,
            inputs: vec![],
            output: Some(ModuleOutput {
                kind: "sf.ethereum.type.v2.Block".to_string(),
            }),
            update_policy: None,
            value_type: None,
        }
    }

    pub fn set_output(&mut self, output_type: &str) {
        let output = output_type.trim_start_matches("proto:");
        self.output = Some(ModuleOutput {
            kind: format!("proto:{output}"),
        });
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

    pub fn store_kind(&self) -> Option<&'static str> {
        if let Some(update_policy) = &self.update_policy {
            match update_policy {
                UpdatePolicy::Set => return Some("StoreSetProto<JsonValue>"),
                UpdatePolicy::SetIfNotExists => return Some("StoreSetIfNotExistsProto<JsonValue>"),
                UpdatePolicy::Add => return Some("StoreAddBigInt"),
            }
        }
        None
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

        Self {
            modules: module_map,
        }
    }

    fn input_names(inputs: Array) -> Vec<String> {
        inputs
            .into_iter()
            .map(|e| from_dynamic(&e).expect("Should be a list of strings"))
            .collect::<Vec<String>>()
    }

    fn module_inputs(&self, input_names: &Vec<String>) -> Vec<ModuleInput> {
        input_names
            .iter()
            .filter_map(|input| {
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
                    s if s.ends_with(":set") || s.ends_with(":setOnce") || s.ends_with(":add") => {
                        // we skip the store modules for inputs
                        return None;
                    }
                    _ => {
                        name = input;
                    }
                }

                let module = self
                    .get_module(&name)
                    .expect(&format!("No module found with name {:?}", input));

                Some((module, access_mode))
            })
            .map(|(module, access_mode)| match module.kind() {
                ModuleKind::Map => ModuleInput::map(module.name().to_string()),
                ModuleKind::Store => {
                    let value_type = match module.update_policy.unwrap() {
                        UpdatePolicy::Set | UpdatePolicy::SetIfNotExists => "JsonValue".to_string(),
                        UpdatePolicy::Add => "BigInt".to_string(),
                    };
                    ModuleInput::store(module.name().to_string(), access_mode, value_type)
                }
                ModuleKind::Source => ModuleInput::eth_block(),
            })
            .collect::<Vec<_>>()
    }

    fn update_policy(inputs: &Array) -> UpdatePolicy {
        let last_input = inputs
            .last()
            .and_then(|e| e.clone().into_string().ok())
            .expect("Empty inputs for a store module! There must be at least 2 inputs. One for the data incoming, and another as the api for the store.");

        let update_policy = last_input.split(":").last()
            .expect("SYNTAX ERROR! The last input to a store module, must be of the form: ident:[set|setOnce|add]");

        match update_policy {
            "set" => UpdatePolicy::Set,
            "setOnce" => UpdatePolicy::SetIfNotExists,
            "add" => UpdatePolicy::Add,
            _ => panic!("Unknown update policy!"),
        }
    }

    pub fn new_shared() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::new()))
    }

    pub fn add_mfn(&mut self, name: String, inputs: Array) {
        let input_names = Self::input_names(inputs);
        let inputs = self.module_inputs(&input_names);

        self.modules
            .insert(name.clone(), ModuleData::new_mfn(name, inputs));
    }

    pub fn add_sfn(&mut self, name: String, inputs: Array) {
        let update_policy = Self::update_policy(&inputs);
        let input_names = Self::input_names(inputs);
        let inputs = self.module_inputs(&input_names);

        self.modules.insert(
            name.clone(),
            ModuleData::new_sfn(name, inputs, update_policy),
        );
    }

    pub fn add_sink(&mut self, kind: &str, inputs: Array) {}

    pub fn get_module(&self, name: &str) -> Option<&ModuleData> {
        self.modules.get(name)
    }

    pub fn generate_streamline_modules(&self, sink_config: &GlobalSinkConfig) -> String {
        let modules = self.modules.values().collect::<Vec<_>>();
        codegen::rust::generate_streamline_modules(&modules, sink_config)
    }
}

pub type GlobalModuleDag = Rc<RefCell<ModuleDag>>;

/// The `Modules` module provides functionality for managing the module dependency graph.
#[export_module]
pub mod module_api {
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
}

pub fn init_globals(engine: &mut Engine, scope: &mut Scope) {
    let module_dag = ModuleDag::new_shared();
    let sink_config_map = SinkConfigMap::new_shared();

    let modules = module_dag.clone();
    // TODO - change this to accept in an array of strings, which we will look up to resolve input types
    engine.register_fn("add_mfn", move |name: Dynamic, inputs: Dynamic| {
        let name = from_dynamic(&name).unwrap();
        let inputs = from_dynamic(&inputs).unwrap();

        (*modules).borrow_mut().add_mfn(name, inputs);
        "Added mfn to DAG!".to_string()
    });

    let modules = module_dag.clone();
    engine.register_fn("add_sfn", move |name: Dynamic, inputs: Array| {
        let name = from_dynamic(&name).unwrap();
        (*modules).borrow_mut().add_sfn(name, inputs);
        "Added sfn to DAG!".to_string()
    });

    let modules = module_dag.clone();
    let sink_config = sink_config_map.clone();
    engine.register_fn("generate_yaml", move |path: String| {
        let modules = (*modules).borrow();

        let yaml = codegen::yaml::generate_yaml(&modules, &sink_config);
        fs::write(&path, &yaml).unwrap();
        format!("Wrote yaml to {} successfully!", &path)
    });

    let modules = module_dag.clone();
    let sink_config = sink_config_map.clone();
    engine.register_fn("generate_rust", move |path: String| {
        let modules_source = (*modules)
            .borrow()
            .generate_streamline_modules(&sink_config);
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
