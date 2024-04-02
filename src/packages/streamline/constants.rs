pub const MFN_OUTPUT: &str = "-> Option<JsonValue>";
pub const MFN_OUTPUT_TYPE: &str = "JsonValue";
pub const MFN_ATTRIBUTE: &str = "#[substreams::handlers::map]";
pub const MFN_DEFAULT_CONVERSION: &str = r#"serde_json::from_value(serde_json::to_value(&result).expect("Couldn't convert from Dynamic!")).expect("Failed to convert output_map to json")"#;
pub const SFN_ATTRIBUTE: &str = "#[substreams::handlers::store]";

pub const SFN_JSON_GET: &str = "StoreGetProto<JsonValue>";
pub const SFN_JSON_DELTAS: &str = "Deltas<DeltaProto<JsonValue>>";

pub const SFN_BIGINT_GET: &str = "StoreGetBigInt";
pub const SFN_BIGINT_DELTAS: &str = "Deltas<DeltaBigInt>";

pub const BIGINT_PROTO: &str = "bigint";
pub const JSON_VALUE_PROTO: &str = "proto:google.protobuf.Value";
pub const INITIAL_BLOCK: Option<i64> = Some(72491700);
