use serde::{Deserialize, Serialize};

use crate::ImmutableString;

use super::{
    constants::{BIGINT_PROTO, INITIAL_BLOCK, JSON_VALUE_PROTO},
    modules::{Input, Kind},
};

pub enum AccessMode {
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

impl From<Kind> for ModuleKind {
    fn from(value: Kind) -> Self {
        match value {
            Kind::Map => Self::Map,
            Kind::Store => Self::Store,
        }
    }
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
