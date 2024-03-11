use crate::{plugin::*, Array, Map as Obj, Scope};
use core::convert::TryFrom;
use core::str::FromStr;
use num_traits::ToPrimitive;
use substreams::scalar::BigInt;
use substreams_entity_change::pb::entity::value::Typed;

use substreams_entity_change::pb::entity::{Array as SfArray, EntityChange, Field, Value};

macro_rules! set {
    ($map:ident, $key:expr, $val:expr) => {
        $map.insert($key.into(), $val.into());
    };
}

#[export_module]
pub mod graph_out {
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
        set!(obj, "new_value", value);
        set!(obj, "subgraph_variant", variant);

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

pub fn init_globals(engine: &mut Engine, _scope: &mut Scope) {
    let module = exported_module!(graph_out);
    engine.register_static_module("subgraph_helpers", module.into());
}
