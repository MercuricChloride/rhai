use crate::serde::from_dynamic;
use crate::{plugin::*, Scope};
use core::cell::RefCell;
use core::convert::TryInto;
use core::str::FromStr;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::sync::Arc;
use std::{collections::BTreeMap, fs};
use substreams::scalar::BigInt;
use substreams::Hex;
use substreams_entity_change::pb::entity::value::Typed;

use substreams_entity_change::pb::entity::{Array, EntityChange, Field, Value};
use substreams_entity_change::tables::{Row, Tables};

#[export_module]
pub mod graph_out {
    use substreams_entity_change::pb::entity::entity_change::Operation;

    pub type SubgraphFieldChange = Field;

    pub fn create_entity(
        entity_name: String,
        entity_id: String,
        fields: Vec<SubgraphFieldChange>,
    ) -> EntityChange {
        EntityChange {
            entity: entity_name,
            id: entity_id,
            ordinal: 0,
            operation: Operation::Create.into(),
            fields,
        }
    }

    #[rhai_fn(global)]
    pub fn update_entity(
        entity_name: String,
        entity_id: String,
        fields: Vec<SubgraphFieldChange>,
    ) -> EntityChange {
        EntityChange {
            entity: entity_name,
            id: entity_id,
            ordinal: 0,
            operation: Operation::Update.into(),
            fields,
        }
    }

    #[rhai_fn(global)]
    pub fn delete_entity(entity_name: String, entity_id: String) -> EntityChange {
        EntityChange {
            entity: entity_name,
            id: entity_id,
            ordinal: 0,
            operation: Operation::Delete.into(),
            fields: vec![],
        }
    }

    #[rhai_fn(global)]
    pub fn field_change(name: String, value: Dynamic, variant: String) -> Field {
        Field {
            name,
            old_value: None,
            new_value: dynamic_into_subgraph_value(value, &variant),
        }
    }
}

macro_rules! as_value {
    ($variant:ident, $value:expr) => {
        Some(Value {
            typed: Some(Typed::$variant($value)),
        })
    };
}

fn dynamic_into_subgraph_value(value: Dynamic, variant: &str) -> Option<Value> {
    let error_msg = format!(
        "Failed converting value {:?}, into a subgraph type!",
        &value
    );

    if value.is_bool() {
        let value: bool = value.cast();
        return as_value!(Bool, value);
    }

    if value.is_int() {
        let value = value.as_int().ok().and_then(|value| value.to_i32());
        if let Some(value) = value {
            return as_value!(Int32, value);
        }
    }

    if value.is_array() {
        if let Ok(value) = value.into_array() {
            let value = value
                .into_iter()
                .filter_map(|item| dynamic_into_subgraph_value(item, variant))
                .collect::<Vec<_>>();
            return as_value!(Array, Array { value });
        }
    } else {
        let value = value.into_string();

        if let Ok(value) = value {
            match variant {
                "ADDRESS" => {
                    return as_value!(Bytes, value);
                }
                "BIGINT" => {
                    let as_bigint = BigInt::from_str(&value).ok()?;
                    return as_value!(Bigint, as_bigint.to_string());
                }
                "BIGDECIMAL" => {
                    todo!("Big decimal isn't supported yet!");
                    //let as_bigint = BigInt::from_str(&value).ok()?;
                    //return as_value!(Bigint, as_bigint.to_string());
                }
                "STRING" => {
                    return as_value!(String, value);
                }
                "BYTES" => {
                    return as_value!(Bytes, value);
                }
                _ => {}
            }
        }
    }
    substreams::log::println(error_msg);
    None
}

pub fn init_globals(engine: &mut Engine, _scope: &mut Scope) {
    let module = exported_module!(graph_out);
    engine.register_static_module("subgraph_helpers", module.into());
}
