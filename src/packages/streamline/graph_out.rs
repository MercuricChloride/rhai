use crate::serde::from_dynamic;
use crate::{plugin::*, Array, Map as Obj, Scope};
use anyhow::{anyhow, bail, ensure, Error};
use core::convert::{TryFrom, TryInto};
use core::str::FromStr;
use num_traits::ToPrimitive;
use substreams::scalar::{BigDecimal, BigInt};
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

fn to_string(value: Dynamic) -> Result<String, Error> {
    match value.type_name() {
        "Uint" => {
            let error = anyhow!("failed to cast {:?} to BigInt", value);
            let big_int = value.try_cast::<BigInt>();
            ensure!(big_int.is_some(), error);
            return Ok(big_int.unwrap().to_string());
        }
        "Address" => {
            let error = anyhow!("failed to cast {:?} to address (bytes)", value);
            let as_bytes = value.try_cast::<Vec<u8>>();
            ensure!(as_bytes.is_some(), error);
            let as_hex = Hex(as_bytes.unwrap()).to_string();
            return Ok(format!("0x{}", as_hex));
        }
        _ => {
            let error = anyhow!("Failed to cast {:?} as a String!", &value);
            let value = value.into_string();
            ensure!(value.is_ok(), error);
            return Ok(value.unwrap());
        }
    }
}

fn to_hex_string(value: Dynamic) -> Result<String, Error> {
    match value.type_name() {
        "Uint" => {
            let error = anyhow!("failed to cast {:?} to BigInt", value);
            let big_int = value.try_cast::<BigInt>();
            ensure!(big_int.is_some(), error);
            // TODO Pretty sure this is right, though it depends on how the subgraph sink interprets bytes
            let as_hex = Hex::encode(big_int.unwrap().to_signed_bytes_be());
            return Ok(format!("0x{}", as_hex));
        }
        "Address" => {
            let error = anyhow!("failed to cast {:?} to address (bytes)", value);
            let as_bytes = value.try_cast::<Vec<u8>>();
            ensure!(as_bytes.is_some(), error);
            let as_hex = Hex(as_bytes.unwrap()).to_string();
            return Ok(format!("0x{}", as_hex));
        }
        _ => {
            todo!("Need to add support for other types as a Hex String")
            //let error = anyhow!("Failed to cast {:?} as a String!", &value);
            //let value = value.into_string();
            //ensure!(value.is_ok(), error);
            //return Ok(value.unwrap());
        }
    }
}

impl TryInto<BigInt> for Dynamic {
    type Error = Error;

    fn try_into(self) -> Result<BigInt, Self::Error> {
        let error = anyhow!("Couldn't convert {:?} into BigInt!", &self.type_name());

        let big_int = match self.type_name() {
            "Uint" | "substreams::scalar::BigInt" => self.try_cast::<BigInt>(),
            "i64" => Some(BigInt::from(self.cast::<i64>())),
            "i32" => Some(BigInt::from(self.cast::<i32>())),
            "string" => {
                let as_string = &self.into_immutable_string().unwrap();

                if as_string.is_empty() {
                    Some(BigInt::zero())
                } else {
                    Some(BigInt::from_str(as_string).unwrap_or_else(|e| {
                        substreams::log::println(&error.to_string());
                        BigInt::zero()
                    }))
                }
            }
            "map" => {
                let mut obj: Obj = self.cast();
                let value_type = obj
                    .remove("type")
                    .and_then(|e| e.into_string().ok())
                    .unwrap();

                let value = obj
                    .remove("value")
                    .and_then(|e| e.into_string().ok())
                    .unwrap();

                if &value_type == "Uint" {
                    Some(BigInt::from_str(&value).unwrap())
                } else {
                    bail!("asldfkjasf")
                }
            }
            _ => bail!(error),
        };

        Ok(big_int.unwrap())
    }
}

impl TryInto<BigDecimal> for Dynamic {
    type Error = Error;

    fn try_into(self) -> Result<BigDecimal, Self::Error> {
        if self.is_string() {
            let string = self.to_string();
            if let Ok(value) = BigDecimal::from_str(&string) {
                return Ok(value);
            }

            if let Ok(value) = BigInt::from_str(&string) {
                return Ok(value.into());
            }

            bail!(
                "Failed to convert {:?} to a BigDecimal in graph_out!",
                string
            );
        } else {
            let big_int: BigInt = self.try_into()?;
            Ok(big_int.into())
        }
    }
}

impl TryInto<Value> for Dynamic {
    type Error = Error;

    fn try_into(self) -> Result<Value, Self::Error> {
        let mut value: Obj = self.cast();
        let variant = value.remove("variant");
        let value = value.remove("value");

        if let (Some(variant), Some(value)) = (variant, value) {
            let variant: String = variant.cast();
            match variant.as_str() {
                s if s.starts_with("BigInt") => {
                    let value: BigInt = value.try_into()?;
                    return field_value!(Bigint, value.to_string());
                }
                s if s.starts_with("Address") || s.starts_with("String") => {
                    let value = to_string(value)?;
                    return field_value!(String, value);
                }
                s if s.starts_with("Bytes") => {
                    let value = to_hex_string(value)?;
                    return field_value!(Bytes, value);
                }
                s if s.starts_with("Bool") => {
                    if value.is_bool() {
                        return field_value!(Bool, value.as_bool().map_err(|e| anyhow!("{}", e))?);
                    }
                }
                s if s.starts_with("Array") => todo!("Not supported yet!"),
                s if s.starts_with("BigDecimal") => {
                    let value: BigDecimal = value.try_into()?;
                    return field_value!(Bigdecimal, value.to_string());
                }
                _ => {
                    let value = to_string(value)?;
                    return field_value!(String, value);
                }
            }
        }
        Err(anyhow!("Couldn't convert value into FieldValue"))
    }
}

fn as_field(mut change: Obj) -> Option<Field> {
    let name: String = change.remove("name")?.try_cast()?;
    let value_obj = change.remove("new_value")?;
    let new_value = value_obj.try_into().unwrap();

    Some(Field {
        name,
        new_value: Some(new_value),
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
