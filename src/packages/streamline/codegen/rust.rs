use super::Codegen;
use crate::packages::streamline;
use crate::packages::streamline::constants::{
    MFN_ATTRIBUTE, MFN_DEFAULT_CONVERSION, MFN_OUTPUT, MFN_OUTPUT_TYPE, SFN_ATTRIBUTE,
    SFN_BIGINT_DELTAS, SFN_BIGINT_GET, SFN_JSON_DELTAS, SFN_JSON_GET,
};
use crate::packages::streamline::modules::{Input, Kind, UpdatePolicy};
use crate::ImmutableString;
use std::rc::Rc;
use streamline::modules as m; //::{Accessor, Kind, Input as SInput, Module as SModule};
use streamline::sink::{DefaultModuleResolver, ModuleResolver, ResolvedModule};

macro_rules! multi_let {
    ($($ident:ident),+) => {
        $(let $ident;)+
    };
}

pub struct RustGenerator {
    functions: Vec<RustHandler>,
}

impl RustGenerator {
    pub fn new(modules: Vec<ImmutableString>, resolver: Box<dyn ModuleResolver>) -> Self {
        let modules = modules
            .into_iter()
            .filter_map(|e| resolver.get(e))
            .collect::<Vec<_>>();
    }
}

/// Contains all the data to generate a single rust fn for a module
struct RustHandler {
    name: ImmutableString,
    inputs: Vec<Box<dyn Codegen>>,
    conversion: ImmutableString,
    output_type: ImmutableString,
    attribute: ImmutableString,
}

/// This represents a single function input in the rust handler
struct RustInput {
    name: ImmutableString,
    value_type: ImmutableString,
}

impl RustInput {
    pub fn new(input: &Input, resolver: Box<dyn ModuleResolver>) -> Self {
        let Input { name, access } = &input;

        let resolved = resolver
            .get(name.into())
            .expect(&format!("No module found for: {}", &name));

        match resolved {
            ResolvedModule::Module(module) => {
                if let Kind::Map = module.kind {
                    RustInput {
                        name: name.into(),
                        value_type: MFN_OUTPUT_TYPE.into(),
                    }
                } else {
                    let update_policy = module
                        .update_policy()
                        .expect("Store value didn't have an update policy!");

                    let value_type = match update_policy {
                        UpdatePolicy::Add => match access {
                            m::Accessor::Deltas => SFN_BIGINT_DELTAS,
                            m::Accessor::Get | m::Accessor::Default => SFN_BIGINT_GET,
                            m::Accessor::Store(_) => unreachable!(),
                        },
                        _ => match access {
                            m::Accessor::Deltas => SFN_JSON_DELTAS,
                            m::Accessor::Get | m::Accessor::Default => SFN_JSON_GET,
                            m::Accessor::Store(_) => unreachable!(),
                        },
                    };

                    RustInput {
                        name: name.into(),
                        value_type: value_type.into(),
                    }
                }
            }
            ResolvedModule::SinkConfig(sink) => RustInput {
                name: name.into(),
                value_type: sink.rust_name.as_str().into(),
            },
            ResolvedModule::Source(source) => RustInput {
                name: name.into(),
                value_type: source.rust_name.as_str().into(),
            },
        }
    }
}

impl RustHandler {
    pub fn new(
        name: ImmutableString,
        resolver: Box<dyn ModuleResolver>,
        inputs: Vec<Box<dyn Codegen>>,
    ) -> Self {
        let module = resolver
            .get(name.clone())
            .expect(&format!("No module found for: {}", &name));

        match module {
            ResolvedModule::Module(module) => {
                if let Kind::Map = module.kind {
                    RustHandler {
                        name,
                        inputs,
                        conversion: MFN_DEFAULT_CONVERSION.into(),
                        output_type: MFN_OUTPUT.into(),
                        attribute: MFN_ATTRIBUTE.into(),
                    }
                } else {
                    RustHandler {
                        name,
                        inputs,
                        conversion: "".into(),
                        output_type: "".into(),
                        attribute: SFN_ATTRIBUTE.into(),
                    }
                }
            }
            ResolvedModule::SinkConfig(sink) => RustHandler {
                name,
                inputs,
                conversion: sink.fully_qualified_path.as_str().into(),
                output_type: sink.rust_name.as_str().into(),
                attribute: MFN_ATTRIBUTE.into(),
            },
            ResolvedModule::Source(_) => unreachable!(),
        }
    }
}

impl Codegen for RustHandler {
    fn generate(&self) -> String {
        let Self {
            name,
            inputs,
            conversion,
            output_type,
            attribute,
        } = &self;
        // Store modules don't do anything with the result of the function call, so we set the 'body' to be an empty string
        // Otherwise we have to apply some conversions to them
        let mut body: String = "".into();

        let inputs = inputs.iter().map(|e| e.generate()).collect::<Vec<_>>();
        // We need to track if there is a single input to the module, so we can add the extra comma to the end of the tuple in the rust code
        // (foo) evaluates to foo
        // (foo,) is a single len tuple containing foo
        let single_input = inputs.len() == 1;

        let fn_inputs = &inputs.join(",");
        let mut handler_inputs = inputs
            .clone()
            .iter()
            .map(|input| {
                // inputs are of the form
                // name: Type
                // and we only need the name
                input.split(":").collect::<Vec<_>>()[0]
            })
            .collect::<Vec<_>>()
            .join(",");

        if single_input {
            handler_inputs.push_str(",");
        }

        // if the output_type isn't empty, it means its a map module, so we need to do something with the result
        // of the function call
        if !output_type.is_empty() {
            body = format!(
                r#"
if result.is_unit() {{
    None
}} else {{
    let result = {conversion};
    Some(result)
}}
"#
            );
        }

        format!(
            r#"
{attribute}
fn {name}({fn_inputs}) {output_type} {{
    let (mut engine, mut scope) = engine_init!();
    let ast = engine.compile(RHAI_SCRIPT).unwrap();
    let result: Dynamic = engine.call_fn(&mut scope, &ast, "{name}", ({handler_inputs})).expect("Call failed");
    {body}
}}
"#,
        )
    }
}

impl Codegen for RustGenerator {
    fn generate(&self) -> String {
        let Self { functions } = &self;
        functions
            .iter()
            .map(Codegen::generate)
            .collect::<Vec<_>>()
            .join("")
    }
}

// impl Codegen for RustGenerator {
//     fn generate(&self) -> String {
//         let attribute;
//         let mut output_type;

//         if let Kind::Map = &self.kind {
//             attribute = "#[substreams::handlers::map]".to_string();
//             output_type = "-> Option<JsonValue>".to_string();
//         } else {
//             attribute = "#[substreams::handlers::store]".to_string();
//             output_type = "".to_string();
//         }

//         let Module { name, inputs: m_inputs , kind: m_kind } = &self;

//         let inputs = m_inputs.iter().map(|e| e.generate(resolver)).collect::<Vec<_>>().join(",");

//         if let Some(ResolvedModule::Sink(config)) = resolver.get(name.clone()) {
//             // Sink stuff
//             todo!()
//         } else {
//             todo!()
//             // normal
//         }

//         format!(r#"\
// {{attribute}}
// fn {name}({inputs}) {output_type} {{
//     {formatters}
//     let (mut engine, mut scope) = engine_init!();
//     let ast = engine.compile(RHAI_SCRIPT).unwrap();
//     let result: Dynamic = engine.call_fn(&mut scope, &ast, "{handler}", ({args})).expect("Call failed");
//     if result.is_unit() {{
//         None
//     }} else {{
//         let conversion = {fully_qualified_path}(result);
//         Some(conversion)
//     }}
// }}
//             "#)
//     }
// }

pub struct RustModuleTemplate {
    name: ImmutableString,
    inputs: Vec<Input>,
}

pub fn generate_rust_module(name: ImmutableString, inputs: Vec<Input>) -> String {
    todo!()
}

pub mod rust {
    use crate::packages::streamline::{
        modules::{Kind, ModuleInput},
        sink::{GlobalSinkConfig, SinkConfig},
    };

    use super::*;

    trait Generate {
        fn generate(&self) -> String;
        fn generate_formatters(&self) -> Option<String>;
    }

    impl Generate for ModuleInput {
        fn generate(&self) -> String {
            match self {
                ModuleInput::Map { map: name } => {
                    format!("{name}: JsonValue")
                }

                ModuleInput::Store {
                    store: name,
                    mode,
                    value_type,
                } => match mode.as_str() {
                    "get" => match value_type.as_str() {
                        "BigInt" => format!("{name}: StoreGetBigInt"),
                        _ => format!("{name}: StoreGetProto<JsonValue>"),
                    },
                    "deltas" => match value_type.as_str() {
                        "BigInt" => format!("{name}: Deltas<DeltaBigInt>"),
                        _ => format!("{name}: Deltas<DeltaProto<JsonValue>>"),
                    },
                    _ => panic!("Unknown mode"),
                },

                ModuleInput::Source { source } => "block: EthBlock".to_string(),
            }
        }

        fn generate_formatters(&self) -> Option<String> {
            match self {
                ModuleInput::Store {
                    store: name, mode, ..
                } => {
                    if mode.as_str() == "deltas" {
                        return Some(format!("let {name} = Rc::new({name});"));
                    }

                    if mode.as_str() == "get" {
                        return Some(format!("let {name} = Rc::new({name});"));
                    }
                }
                _ => {}
            }
            None
        }
    }

    impl Generate for &Vec<ModuleInput> {
        fn generate(&self) -> String {
            self.iter()
                .map(|i| i.generate())
                .collect::<Vec<_>>()
                .join(", ")
        }

        fn generate_formatters(&self) -> Option<String> {
            let formatters = self
                .iter()
                .filter_map(|i| i.generate_formatters())
                .collect::<Vec<_>>();

            if formatters.is_empty() {
                None
            } else {
                Some(formatters.join("\n"))
            }
        }
    }

    pub fn generate_streamline_modules(
        modules: &Vec<&ModuleData>,
        sink_config: &GlobalSinkConfig,
    ) -> String {
        modules.iter().fold(String::new(), |acc, e| {
            let module_code = match e.kind() {
                Kind::Map => {
                    if let Some(config) = sink_config.borrow().sinks.get(e.name()) {
                        generate_sink(e.name(), e.inputs(), e.handler(), config)
                    } else {
                        generate_mfn(e.name(), e.inputs(), e.handler())
                    }
                }
                Kind::Store => {
                    let store_kind = e
                        .store_kind()
                        .expect("Tried to get store_kind for a module that wasn't set / setOnce");
                    generate_sfn(e.name(), store_kind, e.inputs(), e.handler())
                }
                _ => "".to_string(),
            };
            format!("{acc}{module_code}")
        })
    }

    /// This function matches against the name of the map module
    /// and uses the appropriate conversion function for the type of sink it is
    fn get_output_type(name: &str) -> &'static str {
        match name {
            "graph_out" => "EntityChanges",
            _ => "JsonValue",
        }
    }

    fn generate_sink(
        name: &str,
        inputs: &Vec<ModuleInput>,
        handler: &str,
        config: &SinkConfig,
    ) -> String {
        let SinkConfig {
            crate_name,
            fully_qualified_path,
            ..
        } = config;

        let output_type = get_output_type(name);
        let module_inputs = inputs.generate();
        let formatters = if let Some(val) = inputs.generate_formatters() {
            val
        } else {
            String::new()
        };
        let arg_names = inputs
            .iter()
            .map(|input| {
                let name = input.name();
                match &input {
                    ModuleInput::Map { .. } => {
                        format!("to_dynamic(serde_json::to_value({name}).unwrap()).unwrap()")
                    }
                    _ => name,
                }
            })
            .collect::<Vec<_>>();

        let args = if arg_names.len() == 1 {
            format!("{},", arg_names[0])
        } else {
            arg_names.join(",")
        };

        format!(
            r#"
#[substreams::handlers::map]
fn {name}({module_inputs}) -> Option<{output_type}> {{
    {formatters}
    let (mut engine, mut scope) = engine_init!();
    let ast = engine.compile(RHAI_SCRIPT).unwrap();
    let result: Dynamic = engine.call_fn(&mut scope, &ast, "{handler}", ({args})).expect("Call failed");
    if result.is_unit() {{
        None
    }} else {{
        let conversion = {fully_qualified_path}(result);
        Some(conversion)
    }}
}}
    "#
        )
    }

    fn generate_mfn(name: &str, inputs: &Vec<ModuleInput>, handler: &str) -> String {
        // The rust fn inputs
        let output_type = get_output_type(name);
        let module_inputs = inputs.generate();
        let formatters = if let Some(val) = inputs.generate_formatters() {
            val
        } else {
            String::new()
        };

        let arg_names = inputs
            .iter()
            .map(|input| {
                let name = input.name();
                match &input {
                    ModuleInput::Map { .. } => {
                        format!("to_dynamic(serde_json::to_value({name}).unwrap()).unwrap()")
                    }
                    ModuleInput::Store { .. } => name,
                    ModuleInput::Source { .. } => name,
                }
            })
            .collect::<Vec<_>>();

        let args = if arg_names.len() == 1 {
            format!("{},", arg_names[0])
        } else {
            arg_names.join(",")
        };

        format!(
            r#"
#[substreams::handlers::map]
fn {name}({module_inputs}) -> Option<{output_type}> {{
    {formatters}
    let (mut engine, mut scope) = engine_init!();
    let ast = engine.compile(RHAI_SCRIPT).unwrap();
    let result: Dynamic = engine.call_fn(&mut scope, &ast, "{handler}", ({args})).expect("Call failed");
    if result.is_unit() {{
        None
    }} else {{
        let result = serde_json::to_value(&result).expect("Couldn't convert from Dynamic!");
        Some(serde_json::from_value(result).expect("Failed to convert output_map to json"))
    }}
}}
    "#
        )
    }

    fn generate_sfn(
        name: &str,
        store_kind: &str,
        inputs: &Vec<ModuleInput>,
        handler: &str,
    ) -> String {
        let module_inputs = inputs.generate();
        let formatters = if let Some(val) = inputs.generate_formatters() {
            val
        } else {
            String::new()
        };
        let args = inputs
            .iter()
            .map(|input| {
                let name = input.name();
                match &input {
                    ModuleInput::Map { .. } => {
                        format!("to_dynamic(serde_json::to_value({name}).unwrap()).unwrap()")
                    }
                    ModuleInput::Store { .. } => name,
                    ModuleInput::Source { .. } => name,
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            r#"
#[substreams::handlers::store]
fn {name}({module_inputs}, streamline_store_param: {store_kind}) {{
    let streamline_store_param = Rc::new(streamline_store_param);
    {formatters}
    let (mut engine, mut scope) = engine_init!();
    let ast = engine.compile(RHAI_SCRIPT).unwrap();
    let result:Dynamic = engine.call_fn(&mut scope, &ast, "{handler}", ({args}, streamline_store_param)).expect("Call failed");
}}
    "#
        )
    }
}

pub mod yaml {
    use crate::packages::streamline::{
        modules::{Kind, ModuleDag},
        sink::GlobalSinkConfig,
    };

    const TEMPLATE_YAML: &'static str = "
specVersion: v0.1.0
package:
  name: erc721
  version: v0.1.0

imports:
  sql: https://github.com/streamingfast/substreams-sink-sql/releases/download/protodefs-v1.0.2/substreams-sink-sql-protodefs-v1.0.2.spkg
  database_change: https://github.com/streamingfast/substreams-sink-database-changes/releases/download/v1.2.1/substreams-database-change-v1.2.1.spkg
  entity: https://github.com/streamingfast/substreams-entity-change/releases/download/v0.2.1/substreams-entity-change-v0.2.1.spkg

protobuf:
  files:
   - struct.proto
  importPaths:
    - ./proto

network: mainnet

binaries:
  default:
    type: wasm/rust-v1
    file: ./target/wasm32-unknown-unknown/release/streamline.wasm

modules:
$$MODULES$$
";

    pub fn generate_yaml(modules: &ModuleDag, sink_config: &GlobalSinkConfig) -> String {
        // we are cloning here because this only runs at compile time, so it's fine
        let mut modules = modules.modules.clone();
        let modules = modules
            .iter_mut()
            .filter_map(|(_, module)| match module.kind() {
                Kind::Map => {
                    if let Some(config) = sink_config.borrow().sinks.get(module.name()) {
                        module.set_output(&config.protobuf_name);
                        Some(module)
                    } else {
                        Some(module)
                    }
                }
                Kind::Store => Some(module),
                Kind::Source => None,
            })
            .collect::<Vec<_>>();

        let yaml = serde_yaml::to_string(&modules).unwrap();

        TEMPLATE_YAML.replace("$$MODULES$$", &yaml)
    }
}
