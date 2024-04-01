use crate::packages::streamline::modules::{Accessor, Kind};
use crate::ImmutableString;

use super::modules::{Input, Module};
use super::sink::{ModuleResolver, ResolvedModule, DefaultModuleResolver};

macro_rules! multi_let {
    ($($ident:ident),+) => {
        $(let $ident;)+
    };
}

pub trait Codegen<T> {
    fn generate(&self, resolver: Box<dyn ModuleResolver>) -> String;
}

pub struct RustGenerator;

impl Codegen<RustGenerator> for Module {
    fn generate(&self, resolver: Box<dyn ModuleResolver>) -> String {
        let attribute;
        let mut output_type;

        if let Kind::Map = &self.kind {
            attribute = "#[substreams::handlers::map]".to_string();
            output_type = "-> Option<JsonValue>".to_string();
        } else {
            attribute = "#[substreams::handlers::store]".to_string();
            output_type = "".to_string();
        }

        let Module { name, inputs: m_inputs , kind: m_kind } = &self;

        let inputs = m_inputs.iter().map(|e| e.generate(resolver)).collect::<Vec<_>>().join(",");

        if let Some(ResolvedModule::Sink(config)) = resolver.get(name.clone()) {
            // Sink stuff
            todo!()
        } else {
            todo!()
            // normal
        }

        format!(r#"\
{{attribute}}
fn {name}({inputs}) {output_type} {{
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
            "#)
    }
}

impl Codegen<RustGenerator> for Input {
    fn generate(&self, resolver: Box<dyn ModuleResolver>) -> String {
        let Input { name, access } = &self;
        let input_type = match access {
            Accessor::Deltas => "deltas"
            Accessor::Get => todo!(),
            Accessor::Store(_) => todo!(),
            Accessor::Default => todo!(),
        };
        format!("")
    }
}

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
