/// Represents the default output of a MFN module
pub const MFN_OUTPUT: &str = "-> Option<JsonValue>";
/// Represents the default output TYPE of a MFN module
pub const MFN_OUTPUT_TYPE: &str = "JsonValue";
/// Represents the attribute macro on top of every map module
pub const MFN_ATTRIBUTE: &str = "#[substreams::handlers::map]";
/// Represents the default conversion from a Dynamic -> JsonValue
pub const MFN_DEFAULT_CONVERSION: &str = r#"serde_json::from_value(serde_json::to_value(&result).expect("Couldn't convert from Dynamic!")).expect("Failed to convert output_map to json")"#;
/// Represents the attribute macro on top of every store module
pub const SFN_ATTRIBUTE: &str = "#[substreams::handlers::store]";

/// Represents the type of a normal SFN module used in get mode
pub const SFN_JSON_GET: &str = "StoreGetProto<JsonValue>";
/// Represents the type of a normal SFN module used in deltas mode
pub const SFN_JSON_DELTAS: &str = "Deltas<DeltaProto<JsonValue>>";

/// Represents the type of an add SFN module used in get mode
pub const SFN_BIGINT_GET: &str = "StoreGetBigInt";
/// Represents the type of an add SFN module used in deltas mode
pub const SFN_BIGINT_DELTAS: &str = "Deltas<DeltaBigInt>";

/// Represents the protobuf type of a BigInt
pub const BIGINT_PROTO: &str = "bigint";
/// Represents the protobuf type of a JSON_VALUE
pub const JSON_VALUE_PROTO: &str = "proto:google.protobuf.Value";
/// WILL REMOVE: THIS IS THE DEFAULT START BLOCK FOR TESTING
pub const INITIAL_BLOCK: Option<i64> = Some(72491700);
