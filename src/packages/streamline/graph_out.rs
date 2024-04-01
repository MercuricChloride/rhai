use crate::serde::from_dynamic;
use crate::{plugin::*, Array, Map as Obj, Scope};
use core::convert::{TryFrom, TryInto};
use core::str::FromStr;
use num_traits::ToPrimitive;
use substreams::scalar::BigInt;
use substreams::Hex;
use substreams_entity_change::change::ToField;
use substreams_entity_change::pb::entity::value::Typed;

use substreams_entity_change::pb::entity::{
    Array as SfArray, EntityChange, EntityChanges, Field, Value,
};
use substreams_entity_change::tables::ToValue;

macro_rules! set {
    ($map:ident, $key:expr, $val:expr) => {
        $map.insert($key.into(), $val.into());
    };
}

#[export_module]
mod graph_out {
    use substreams_entity_change::pb::entity::entity_change::Operation;

    pub type SubgraphFieldChange = Field;

    #[rhai_fn(global)]
    pub fn create_entity(entity_name: String, entity_id: String, fields: Array) -> Dynamic {
        let mut obj = Obj::new();
        set!(obj, "entity", entity_name);
        set!(obj, "id", entity_id);
        set!(obj, "operation", (Operation::Create as i64));
        set!(obj, "fields", fields);
        obj.into()
    }

    #[rhai_fn(global)]
    pub fn update_entity(entity_name: String, entity_id: String, fields: Array) -> Dynamic {
        let mut obj = Obj::new();
        set!(obj, "entity", entity_name);
        set!(obj, "id", entity_id);
        set!(obj, "operation", (Operation::Update as i64));
        set!(obj, "fields", fields);
        obj.into()
    }

    #[rhai_fn(global)]
    pub fn delete_entity(entity_name: String, entity_id: String) -> Dynamic {
        let mut obj = Obj::new();
        set!(obj, "entity", entity_name);
        set!(obj, "id", entity_id);
        set!(obj, "operation", (Operation::Delete as i64));
        obj.into()
    }

    #[rhai_fn(global)]
    pub fn field_change(name: String, value: Dynamic, variant: String) -> Dynamic {
        let mut obj = Obj::new();
        set!(obj, "name", name);
        let mut value_obj = Obj::new();
        set!(value_obj, "variant", &variant);
        set!(value_obj, "value", value);

        set!(obj, "subgraph_variant", variant);
        set!(obj, "new_value", value_obj);

        obj.into()
    }
}

macro_rules! as_value {
    ($variant:ident, $value:expr) => {
        Some(Value {
            typed: Some(Typed::$variant($value)),
        })
    };
}

macro_rules! field_value {
    ($variant: ident, $value: expr) => {
        Ok(Value {
            typed: Some(Typed::$variant($value)),
        })
    };
}

impl TryInto<Value> for Dynamic {
    type Error = &'static str;

    fn try_into(self) -> Result<Value, Self::Error> {
        let mut value: Obj = self.cast();
        let variant = value.remove("variant");
        let value = value.remove("value");

        if let (Some(variant), Some(value)) = (variant, value) {
            let variant: String = variant.cast();
            match variant.as_str() {
                s if s.starts_with("BigInt") => field_value!(Bigint, value.cast()),
                s if s.starts_with("Address") => {
                    field_value!(String, value.into_string().unwrap())
                }
                _ => todo!(),
            }
        } else {
            Err("Couldn't convert value into FieldValue")
        }
    }
}

fn as_field(mut change: Obj) -> Option<Field> {
    let name: String = change.remove("name")?.try_cast()?;
    let new_value = change.remove("new_value")?.try_into().ok();

    Some(Field {
        name,
        new_value,
        // old_value is deprecated
        old_value: None,
    })
}

fn as_entity_change(mut change: Obj) -> Option<EntityChange> {
    let entity: String = change.remove("entity")?.try_cast()?;
    let id: String = change.remove("id")?.try_cast()?;
    let operation: i64 = change.remove("operation")?.cast();
    let fields: Vec<Field> = change
        .remove("fields")?
        .cast::<Array>()
        .into_iter()
        .filter_map(|item| item.try_cast())
        .filter_map(|mut item| as_field(item))
        .collect();

    Some(EntityChange {
        entity: entity.to_string(),
        id: id.to_string(),
        ordinal: 0, // Not used in graph node
        operation: operation as i32,
        fields,
    })
}

/// Converts an Array of objects, into the EntityChanges protobuf
pub fn as_entity_changes(mut changes: Dynamic) -> EntityChanges {
    if changes.is_array() {
        let changes: Array = changes
            .try_cast::<Array>()
            .expect("Couldn't convert into Array!");

        let entity_changes = changes
            .into_iter()
            .map(|item| item.cast::<Obj>())
            .filter_map(as_entity_change)
            .collect();

        EntityChanges { entity_changes }
    } else {
        substreams::log::println(format!(
            "graph_out output wasn't found to be an array! {changes:?}"
        ));
        EntityChanges {
            entity_changes: vec![],
        }
    }
}

/// Initializes the subgraph_helpers global module for the rhai runtime
pub fn init_globals(engine: &mut Engine, _scope: &mut Scope) {
    let module = exported_module!(graph_out);
    engine.register_static_module("subgraph_helpers", module.into());
}
