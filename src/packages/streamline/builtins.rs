use core::convert::TryFrom;
use core::convert::TryInto;
use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;

use crate::serde::to_dynamic;
use crate::types::dynamic;
use crate::Array;
use crate::Dynamic;
use crate::Engine;
use crate::ImmutableString;
use crate::Map;
use anyhow::anyhow;
use anyhow::bail;
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
use substreams_ethereum::rpc::RPCDecodable;
use substreams_ethereum::rpc::RpcBatch;
use substreams_ethereum::Event;
use substreams_ethereum::Function;

type JsonValue = prost_wkt_types::Value;
type EthBlock = substreams_ethereum::pb::eth::v2::Block;

fn format_hex(address: &[u8]) -> String {
    let address = Hex(address).to_string();
    format!("0x{address}")
}

/// Extracts events of type T, from a block
pub fn get_events<T>(block: &mut EthBlock, addresses: Array) -> Vec<Dynamic>
where
    T: Sized + Event + Clone + Serialize,
{
    let mut address_set = HashSet::new();
    for address in addresses.into_iter() {
        let as_string = address
            .into_string()
            .expect("Address was found to not be a string!");
        address_set.insert(as_string.to_lowercase());
    }

    let mut events = vec![];

    for log in block.logs() {
        let formatted_address = format_hex(log.address());
        if address_set.len() > 0 && !address_set.contains(&formatted_address) {
            continue;
        }

        let tx = log.receipt.transaction;
        let from = format_hex(&tx.from);
        let to = format_hex(&tx.to);
        let tx_hash = format_hex(&tx.hash);
        let block_number = block.number;
        let block_hash = format_hex(&block.hash);

        // TODO Add more metadata
        let tx_meta = json!({
            "address": formatted_address,
            "from": from,
            "to": to,
            "tx_hash": tx_hash,
            "block_number": block_number,
            "block_hash": block_hash
        });

        let event = T::match_and_decode(log);

        if let Some(event) = event {
            let as_value = serde_json::to_value(event);
            match as_value {
                Ok(serde_json::Value::Object(mut val)) => {
                    val.insert("tx_meta".into(), tx_meta);
                    events.push(serde_json::from_value(serde_json::Value::Object(val)).unwrap())
                }
                Err(err) => substreams::log::println(format!(
                    "GOT ERROR CONVERTING EVENT INTO DYNAMIC: {err:?}"
                )),
                _ => substreams::log::println(format!(
                    "EVENT STRUCT NOT FOUND TO BE AN OBJECT!{as_value:?}"
                )),
            }
        }
    }

    events
}

/// Makes an RPC call to an address
pub fn rpc_call<T, E>(kind: T, address: ImmutableString) -> Option<E>
where
    T: Sized + Function + Clone + RPCDecodable<E>,
    E: Clone + 'static,
{
    let address = Hex::decode(address.to_string()).unwrap();
    let batch = RpcBatch::new();

    let call: T = kind.into();
    let responses = batch.add(call, address).execute().unwrap();
    let response = &responses.responses[0];

    RpcBatch::decode::<_, T>(response)
}

trait IntoJson {
    fn into_json(&self) -> serde_json::Value;
}

impl IntoJson for BigInt {
    fn into_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert("type".into(), "Uint".into());
        map.insert("value".into(), self.to_string().into());

        map.into()
    }
}

trait IntoJsonProto {
    fn into_json_proto(self) -> Option<JsonValue>;
}

impl IntoJsonProto for Dynamic {
    fn into_json_proto(self) -> Option<JsonValue> {
        if Dynamic::is::<BigInt>(&self) {
            let value: BigInt = self.cast();
            let as_json = value.into_json();
            return serde_json::from_value(as_json).ok();
        }

        if Dynamic::is::<String>(&self) {
            let value: String = self.cast();
            return Some(value.into());
        }

        if Dynamic::is::<Map>(&self) {
            let value: Map = self.cast();

            let key_vals = value
                .into_iter()
                .filter_map(|(key, value)| {
                    let value = value.into_json_proto();
                    if let Some(value) = value {
                        Some((key.to_string(), value))
                    } else {
                        None
                    }
                })
                .collect::<HashMap<_, _>>();

            return Some(JsonValue::from(key_vals));
        }

        if Dynamic::is::<Array>(&self) {
            let value: Array = self.cast();

            let array = value
                .into_iter()
                .filter_map(|value| {
                    let value = value.into_json_proto();
                    if let Some(value) = value {
                        Some(value)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            return Some(JsonValue::from(array));
        }

        let msg = format!("Unknown Type {:?}", self);
        substreams::log::println(&msg);

        None
    }
}

trait TypeRegister {
    fn register_types(engine: &mut Engine);
}

impl TypeRegister for Rc<Deltas<DeltaProto<JsonValue>>> {
    fn register_types(engine: &mut Engine) {
        engine
            .register_type::<Self>()
            .register_get("deltas", |obj: &mut Rc<Deltas<DeltaProto<JsonValue>>>| {
                let deltas = obj
                    .deltas
                    .iter()
                    .map(|delta| {
                        let new_value = serde_json::to_value(&delta.new_value).unwrap();
                        let new_value: Map = serde_json::from_value(new_value).unwrap();

                        let mut obj = Map::new();
                        obj.insert("operation".into(), (delta.operation as i64).into());
                        obj.insert("ordinal".into(), (delta.ordinal as i64).into());
                        obj.insert("key".into(), delta.key.clone().into());
                        obj.insert(
                            "old_value".into(),
                            to_dynamic(delta.old_value.clone()).unwrap(),
                        );
                        obj.insert("new_value".into(), Dynamic::from_map(new_value));
                        Dynamic::from_map(obj)
                    })
                    .collect::<Vec<Dynamic>>();
                Dynamic::from_array(deltas)
            })
            .register_indexer_get(|obj: &mut Rc<Deltas<DeltaProto<JsonValue>>>, index: i64| {
                obj.clone().deltas[index as usize].clone()
            });
    }
}

impl TypeRegister for Rc<Deltas<DeltaBigInt>> {
    fn register_types(engine: &mut Engine) {
        engine
            .register_type::<Self>()
            .register_get("deltas", |obj: &mut Rc<Deltas<DeltaBigInt>>| {
                let deltas = obj
                    .deltas
                    .iter()
                    .map(|delta| {
                        let new_value = to_dynamic(delta.new_value.into_json());
                        let old_value = to_dynamic(delta.old_value.into_json());

                        let mut obj = Map::new();
                        obj.insert("operation".into(), (delta.operation as i64).into());
                        obj.insert("ordinal".into(), (delta.ordinal as i64).into());
                        obj.insert("key".into(), delta.key.clone().into());
                        obj.insert(
                            "old_value".into(),
                            old_value.unwrap(), //to_dynamic(delta.old_value.to_string()).unwrap(),
                        );
                        obj.insert("new_value".into(), new_value.unwrap());
                        Dynamic::from_map(obj)
                    })
                    .collect::<Vec<Dynamic>>();
                Dynamic::from_array(deltas)
            })
            .register_indexer_get(|obj: &mut Rc<Deltas<DeltaBigInt>>, index: i64| {
                obj.clone().deltas[index as usize].clone()
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
        let prost_to_dynamic = |value: &mut JsonValue| {};
        engine
            .register_type::<JsonValue>()
            .register_indexer_get(|value: &mut JsonValue, property: &str| -> Dynamic {
                let msg = format!("Error converting {:?} into Dynamic!", &value);
                let err_fn = || {
                    substreams::log::println(&msg);
                    println!("{}", &msg);
                    Dynamic::UNIT
                };
                if let Some(kind) = &value.kind {
                    match kind {
                        Kind::NullValue(_) => Dynamic::UNIT,
                        Kind::NumberValue(num) => Dynamic::from_int(num.round() as i64),
                        Kind::BoolValue(boolean) => Dynamic::from_bool(*boolean),
                        Kind::StringValue(string) => Dynamic::from_str(&string).unwrap_or(err_fn()),
                        Kind::ListValue(list) => list
                            .values
                            .iter()
                            .filter_map(|v| to_dynamic(v).ok())
                            .collect::<Vec<Dynamic>>()
                            .into(),
                        Kind::StructValue(object) => {
                            let value = object.fields.get(property).map(|e| to_dynamic(e).unwrap());
                            if let Some(value) = value {
                                value
                            } else {
                                err_fn()
                            }
                        }
                    }
                } else {
                    substreams::log::println("Type not found to have a kind!");
                    Dynamic::UNIT
                }
            })
            .register_fn("to_dynamic", |value: &mut JsonValue| {
                let msg = format!("Error converting {:?} into Dynamic!", &value);
                let err_fn = || {
                    substreams::log::println(&msg);
                    println!("{}", &msg);
                    Dynamic::UNIT
                };
                if let Some(kind) = &value.kind {
                    match kind {
                        Kind::NullValue(_) => Dynamic::UNIT,
                        Kind::NumberValue(num) => Dynamic::from_int(num.round() as i64),
                        Kind::BoolValue(boolean) => Dynamic::from_bool(*boolean),
                        Kind::StringValue(string) => Dynamic::from_str(&string).unwrap_or(err_fn()),
                        Kind::ListValue(list) => list
                            .values
                            .iter()
                            .filter_map(|v| to_dynamic(v).ok())
                            .collect::<Vec<Dynamic>>()
                            .into(),
                        Kind::StructValue(object) => to_dynamic(object).unwrap(),
                    }
                } else {
                    substreams::log::println("Type not found to have a kind!");
                    Dynamic::UNIT
                }
            });
    }
}

impl TypeRegister for Vec<u8> {
    fn register_types(engine: &mut Engine) {
        // register the address type
        engine
            .register_type_with_name::<Vec<u8>>("Address")
            .register_fn("address", |x: ImmutableString| {
                if x.len() == 42 {
                    Dynamic::from(x)
                } else {
                    Dynamic::UNIT
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
            .register_fn("*", |x: BigInt, y: Dynamic| {
                let as_string = y.to_string();
                if let Ok(value) = BigInt::try_from(as_string) {
                    Dynamic::from(x * value)
                } else {
                    Dynamic::from(())
                }
            })
            .register_fn("/", |x: BigInt, y: Dynamic| {
                let as_string = y.to_string();
                if let Ok(value) = BigInt::try_from(as_string) {
                    Dynamic::from(x / value)
                } else {
                    Dynamic::from(())
                }
            })
            .register_fn("+", |x: BigInt, y: Dynamic| {
                let as_string = y.to_string();
                if let Ok(value) = BigInt::try_from(as_string) {
                    Dynamic::from(x + value)
                } else {
                    Dynamic::from(())
                }
            })
            .register_fn("-", |x: BigInt, y: Dynamic| {
                let as_string = y.to_string();
                if let Ok(value) = BigInt::try_from(as_string) {
                    Dynamic::from(x - value)
                } else {
                    Dynamic::from(())
                }
            })
            .register_fn("**", |x: BigInt, y: Dynamic| {
                let as_string = y.to_string();
                if let Ok(value) = as_string.parse() {
                    Dynamic::from(x.pow(value))
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

            if let Some(key) = key.into_string().ok() {
                substreams::log::println(format!("Key: {:?}, Value: {:?}", &key, value));
                if let Some(value) = value.into_json_proto() {
                    store.set(0, key, &value);
                }
            } else {
                substreams::log::println("COULDN'T CONVERT THE KEY INTO A STRING!");
            }
        };

        let set_many_fn = |store: &mut StoreSet, keys: Array, value: Dynamic| {
            let keys: Vec<String> = keys
                .into_iter()
                .filter_map(|e| {
                    if let Some(value) = e.into_string().ok() {
                        Some(value)
                    } else {
                        substreams::log::println("COULDN'T CONVERT THE KEY INTO A STRING!");
                        None
                    }
                })
                .collect::<Vec<_>>();

            let value = value.into_json_proto();
            substreams::log::println(format!("Keys: {:?}, Value: {:?}", &keys, value));
            if let Some(value) = value {
                store.set_many(0, &keys, &value);
            }
        };

        let delete_fn = |store: &mut StoreSet, prefix: Dynamic| {
            if let Some(prefix) = prefix.into_string().ok() {
                store.delete_prefix(0, &prefix)
            } else {
                substreams::log::println("COULDN'T CONVERT THE KEY INTO A STRING!");
            }
        };

        engine
            .register_type_with_name::<Rc<StoreSetProto<JsonValue>>>("StoreSet")
            .register_fn("set", set_fn)
            .register_fn("set_many", set_many_fn)
            .register_fn("delete_prefix", delete_fn);
    }
}

impl TypeRegister for Rc<StoreSetIfNotExistsProto<JsonValue>> {
    fn register_types(engine: &mut Engine) {
        type StoreSetOnce = Rc<StoreSetIfNotExistsProto<JsonValue>>;

        let set_fn = |store: &mut StoreSetOnce, key: Dynamic, value: Dynamic| {
            let error_msg = format!("key: {:?}, value: {:?}", &key, &value);
            if let Some(key) = key.into_string().ok() {
                let value = value.into_json_proto();
                substreams::log::println(format!("Key: {:?}, Value: {:?}", &key, value));
                if let Some(value) = value {
                    store.set_if_not_exists(0, key, &value);
                }
            } else {
                substreams::log::println("COULDN'T CONVERT THE KEY INTO A STRING!");
            }
        };

        let set_many_fn = |store: &mut StoreSetOnce, keys: Array, value: Dynamic| {
            let keys: Vec<String> = keys
                .into_iter()
                .filter_map(|e| {
                    if let Some(value) = e.into_string().ok() {
                        Some(value)
                    } else {
                        substreams::log::println("COULDN'T CONVERT THE KEY INTO A STRING!");
                        None
                    }
                })
                .collect::<Vec<_>>();

            let value = value.into_json_proto();
            if let Some(value) = value {
                store.set_if_not_exists_many(0, &keys, &value);
            }
        };

        let delete_fn = |store: &mut StoreSetOnce, prefix: Dynamic| {
            if let Some(prefix) = prefix.into_string().ok() {
                store.delete_prefix(0, &prefix)
            } else {
                substreams::log::println("COULDN'T CONVERT THE KEY INTO A STRING!");
            }
        };

        engine
            .register_type_with_name::<StoreSetOnce>("StoreSetOnce")
            .register_fn("set", set_fn)
            .register_fn("set_once", set_fn)
            .register_fn("set_many", set_many_fn)
            .register_fn("set_once_many", set_many_fn)
            .register_fn("delete_prefix", delete_fn);
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
    <Rc<Deltas<DeltaProto<JsonValue>>>>::register_types(engine);
    <Rc<Deltas<DeltaBigInt>>>::register_types(engine);
    <Rc<StoreSetProto<JsonValue>>>::register_types(engine);
    <Rc<StoreSetIfNotExistsProto<JsonValue>>>::register_types(engine);
    <Rc<StoreGetProto<JsonValue>>>::register_types(engine);
    <Rc<StoreGetBigInt>>::register_types(engine);
    <Rc<StoreAddBigInt>>::register_types(engine);
}
