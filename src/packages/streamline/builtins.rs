use core::convert::TryFrom;
use std::str::FromStr;

use crate::serde::to_dynamic;
use crate::Array;
use crate::Dynamic;
use crate::Engine;
use crate::Map;
use prost_wkt_types::{value::Kind, Value};
use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;
use serde_json::json;
use std::rc::Rc;
use substreams::pb::substreams::module::Input;
use substreams::pb::substreams::module_progress::Type;
use substreams::prelude::*;
use substreams::Hex;
use substreams_ethereum::Event;

type JsonValue = prost_wkt_types::Value;
type EthBlock = substreams_ethereum::pb::eth::v2::Block;

/// Extracts events of type T, from a block
pub fn get_events<T>(block: &mut EthBlock) -> Vec<Dynamic>
where
    T: Sized + Event + Clone + Serialize,
{
    //let addresses = addresses.iter().map(|address| Hex(address)).collect::<Vec<_>>();
    let mut events = vec![];

    for log in block.logs() {
        let event = T::match_and_decode(log);

        if let Some(event) = event {
            let as_value = serde_json::to_value(event);
            match as_value {
                Ok(val) => {
                    if !val.is_null() {
                        events.push(serde_json::from_value(val).unwrap())
                    }
                }
                Err(err) => substreams::log::println(format!(
                    "GOT ERROR CONVERTING EVENT INTO DYNAMIC: {err:?}"
                )),
            }
        }
    }

    events
}

trait TypeRegister {
    fn register_types(engine: &mut Engine);
}

impl TypeRegister for Deltas<DeltaProto<JsonValue>> {
    fn register_types(engine: &mut Engine) {
        engine.register_type::<Self>().register_get(
            "deltas",
            |obj: &mut Deltas<DeltaProto<JsonValue>>| {
                let deltas = obj
                    .deltas
                    .iter()
                    .map(|delta| {
                        //let old_value = serde_json::to_string(&delta.old_value).unwrap();
                        //let old_value: serde_json::Map<_, _> =
                        //serde_json::from_str(&old_value).unwrap();
                        //let old_value: rhai::Map = serde_json::from_value(old_value).unwrap();

                        let new_value = serde_json::to_value(&delta.new_value).unwrap();
                        let new_value: Map = serde_json::from_value(new_value).unwrap();

                        let mut obj = Map::new();
                        obj.insert("operation".into(), (delta.operation as i64).into());
                        obj.insert("ordinal".into(), (delta.ordinal as i64).into());
                        obj.insert("key".into(), delta.key.clone().into());
                        obj.insert(
                            "oldValue".into(),
                            to_dynamic(delta.old_value.clone()).unwrap(),
                        );
                        obj.insert("newValue".into(), Dynamic::from_map(new_value));
                        Dynamic::from_map(obj)
                    })
                    .collect::<Vec<Dynamic>>();
                Dynamic::from_array(deltas)
            },
        );
    }
}

impl TypeRegister for Deltas<DeltaBigInt> {
    fn register_types(engine: &mut Engine) {
        engine
            .register_type::<Self>()
            .register_get("deltas", |obj: &mut Deltas<DeltaBigInt>| {
                let deltas = obj
                    .deltas
                    .iter()
                    .map(|delta| {
                        //let old_value = serde_json::to_string(&delta.old_value).unwrap();
                        //let old_value: serde_json::Map<_, _> =
                        //serde_json::from_str(&old_value).unwrap();
                        //let old_value: rhai::Map = serde_json::from_value(old_value).unwrap();

                        let new_value = serde_json::to_value(&delta.new_value.to_string()).unwrap();
                        let new_value: Map = serde_json::from_value(new_value).unwrap();

                        let mut obj = Map::new();
                        obj.insert("operation".into(), (delta.operation as i64).into());
                        obj.insert("ordinal".into(), (delta.ordinal as i64).into());
                        obj.insert("key".into(), delta.key.clone().into());
                        obj.insert(
                            "oldValue".into(),
                            to_dynamic(delta.old_value.to_string()).unwrap(),
                        );
                        obj.insert("newValue".into(), Dynamic::from_map(new_value));
                        Dynamic::from_map(obj)
                    })
                    .collect::<Vec<Dynamic>>();
                Dynamic::from_array(deltas)
            });
    }
}

impl TypeRegister for DeltaProto<JsonValue> {
    fn register_types(engine: &mut Engine) {
        engine.register_type::<Self>();
    }
}

impl TypeRegister for JsonValue {
    fn register_types(engine: &mut Engine) {
        engine.register_type::<JsonValue>().register_indexer_get(
            |value: &mut JsonValue, property: &str| -> Dynamic {
                let err_fn = |_| {
                    let msg = format!("Error converting {:?} into Dynamic!", &value);
                    substreams::log::println(&msg);
                    println!("{}", &msg);
                    Dynamic::UNIT
                };
                if let Some(kind) = &value.kind {
                    match kind {
                        Kind::NullValue(_) => Dynamic::UNIT,
                        Kind::NumberValue(num) => Dynamic::from_int(num.round() as i64),
                        Kind::BoolValue(boolean) => Dynamic::from_bool(*boolean),
                        Kind::StringValue(string) => {
                            Dynamic::from_str(&string).unwrap_or_else(err_fn)
                        }
                        Kind::ListValue(list) => Dynamic::from_array(
                            list.values
                                .iter()
                                .filter_map(|v| to_dynamic(v).ok())
                                .collect(),
                        ),
                        Kind::StructValue(object) => {
                            to_dynamic(object).unwrap_or_else(|_| err_fn(()))
                        }
                    }
                } else {
                    Dynamic::UNIT
                }
            },
        );
    }
}

impl TypeRegister for Vec<u8> {
    fn register_types(engine: &mut Engine) {
        // register the address type
        engine
            .register_type_with_name::<Vec<u8>>("Address")
            .register_fn("address", |x: Vec<u8>| {
                if x.len() == 20 {
                    Dynamic::from(format!("0x{}", Hex(x).to_string()))
                } else {
                    Dynamic::from(())
                }
            });
    }
}

impl TypeRegister for BigInt {
    fn register_types(engine: &mut Engine) {
        engine
            .register_type_with_name::<BigInt>("Uint")
            .register_fn("uint", |x: BigInt| x.to_string())
            .register_fn("uint", |x: Dynamic| {
                let as_string = x.to_string();
                if let Ok(value) = BigInt::try_from(as_string) {
                    Dynamic::from(value)
                } else {
                    Dynamic::from(())
                }
            })
            .register_fn("to_string", |x: BigInt| x.to_string());
    }
}

impl TypeRegister for Rc<StoreSetProto<JsonValue>> {
    fn register_types(engine: &mut Engine) {
        type StoreSet = Rc<StoreSetProto<JsonValue>>;
        let set_fn = |store: &mut StoreSet, key: Dynamic, value: Dynamic| {
            let error_msg = format!("Couldn't cast!Key: {:?}, Value: {:?}", &key, value);

            // TODO Add support for storing scalar values
            if let (Some(key), Some(value)) =
                (key.try_cast::<String>(), value.try_cast::<JsonValue>())
            {
                store.set(0, key, &value);
            } else {
                substreams::log::println(error_msg);
            }
        };

        let set_many_fn = |store: &mut StoreSet, keys: Array, value: Dynamic| {
            let keys: Vec<String> = keys
                .into_iter()
                .map(|e| {
                    e.try_cast::<String>()
                        .expect("COULDN'T CONVERT THE KEY INTO A STRING!")
                })
                .collect::<Vec<_>>();

            // TODO Add support for storing scalar values
            if let Some(value) = value.try_cast::<JsonValue>() {
                store.set_many(0, &keys, &value);
            }
        };

        let delete_fn = |store: &mut StoreSet, prefix: Dynamic| {
            if let Some(prefix) = prefix.try_cast::<String>() {
                store.delete_prefix(0, &prefix)
            }
        };

        engine
            .register_type_with_name::<Rc<StoreSetProto<JsonValue>>>("StoreSet")
            .register_fn("set", set_fn)
            .register_fn("setMany", set_many_fn)
            .register_fn("delete_prefix", delete_fn);
    }
}

impl TypeRegister for Rc<StoreSetIfNotExistsProto<JsonValue>> {
    fn register_types(engine: &mut Engine) {
        type StoreSetOnce = Rc<StoreSetIfNotExistsProto<JsonValue>>;

        let set_fn = |store: &mut StoreSetOnce, key: Dynamic, value: Dynamic| {
            let error_msg = format!("key: {:?}, value: {:?}", &key, &value);
            if let (Some(key), Some(value)) =
                (key.try_cast::<String>(), value.try_cast::<JsonValue>())
            {
                substreams::log::println(format!("Key: {:?}, Value: {:?}", &key, value));
                store.set_if_not_exists(0, key, &value);
            } else {
                panic!("{}", error_msg)
            }
        };

        let set_many_fn = |store: &mut StoreSetOnce, keys: Array, value: Dynamic| {
            let keys: Vec<String> = keys
                .into_iter()
                .map(|e| {
                    e.try_cast::<String>()
                        .expect("COULDN'T CONVERT THE KEY INTO A STRING!")
                })
                .collect::<Vec<_>>();

            if let Some(value) = value.try_cast::<JsonValue>() {
                store.set_if_not_exists_many(0, &keys, &value);
            }
        };

        let delete_fn = |store: &mut StoreSetOnce, prefix: Dynamic| {
            if let Some(prefix) = prefix.try_cast::<String>() {
                store.delete_prefix(0, &prefix)
            }
        };

        engine
            .register_type_with_name::<StoreSetOnce>("StoreSetOnce")
            .register_fn("set", set_fn)
            .register_fn("setOnce", set_fn)
            .register_fn("setMany", set_many_fn)
            .register_fn("setOnceMany", set_many_fn)
            .register_fn("deletePrefix", delete_fn);
    }
}

impl TypeRegister for Rc<StoreGetProto<JsonValue>> {
    fn register_types(engine: &mut Engine) {
        engine
            .register_type_with_name::<Self>("StoreGet")
            .register_fn("get", |store: &mut Self, key: String| {
                if let Some(value) = store.get_last(&key) {
                    value
                } else {
                    Default::default()
                }
            })
            .register_fn("get_first", |store: &mut Self, key: String| {
                if let Some(value) = store.get_first(&key) {
                    value
                } else {
                    Default::default()
                }
            });
    }
}

impl TypeRegister for Rc<StoreGetBigInt> {
    fn register_types(engine: &mut Engine) {
        engine
            .register_type_with_name::<Self>("StoreGet")
            .register_fn("get", |store: &mut Self, key: String| {
                if let Some(value) = store.get_last(&key) {
                    value
                } else {
                    Default::default()
                }
            })
            .register_fn("get_first", |store: &mut Self, key: String| {
                if let Some(value) = store.get_first(&key) {
                    value
                } else {
                    Default::default()
                }
            });
    }
}

impl TypeRegister for Rc<StoreAddBigInt> {
    fn register_types(engine: &mut Engine) {
        let add_fn = |store: &mut Self, key: String, value: Dynamic| {
            let error_msg = format!("Couldn't use type: {:?} with store:add", value);

            if value.is_int() {
                let value = BigInt::from(value.as_int().unwrap());
                store.add(0, key, value);
            } else if value.is_string() {
                let value = BigInt::from_str(&value.into_string().unwrap())
                    .expect("Failed to convert string to BigInt!");
                store.add(0, key, value)
            } else {
                substreams::log::println(&error_msg);
                panic!("{}", error_msg);
            }
        };

        let delete_fn = |store: &mut Self, prefix: Dynamic| {
            if let Some(prefix) = prefix.try_cast::<String>() {
                store.delete_prefix(0, &prefix)
            }
        };

        engine
            .register_type_with_name::<Self>("StoreAdd")
            .register_fn("add", add_fn)
            .register_fn("delete_prefix", delete_fn);
    }
}

/// Registers the builtin types with the engine
pub fn register_builtins(engine: &mut Engine) {
    <Vec<u8>>::register_types(engine);
    <BigInt>::register_types(engine);
    <JsonValue>::register_types(engine);
    <Deltas<DeltaProto<JsonValue>>>::register_types(engine);
    <Deltas<DeltaBigInt>>::register_types(engine);
    <Rc<StoreSetProto<JsonValue>>>::register_types(engine);
    <Rc<StoreSetIfNotExistsProto<JsonValue>>>::register_types(engine);
    <Rc<StoreGetProto<JsonValue>>>::register_types(engine);
    <Rc<StoreGetBigInt>>::register_types(engine);
    <Rc<StoreAddBigInt>>::register_types(engine);
}
