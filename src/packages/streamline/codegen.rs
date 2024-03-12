use super::abi::ContractImports;
use super::modules::ModuleData;

pub mod rust {
    use crate::packages::streamline::modules::{ModuleInput, ModuleKind};

    use super::*;

    trait Generate {
        fn generate(&self) -> String;
        fn generate_formatters(&self) -> Option<String>;
    }

    impl Generate for ModuleInput {
        fn generate(&self) -> String {
            match self {
                ModuleInput::Map { map: name } => {
                    format!("{name}: JsonStruct")
                }

                ModuleInput::Store { store: name, mode } => match mode.as_str() {
                    "get" => {
                        format!("{name}: StoreGetProto<JsonStruct>")
                    }
                    "deltas" => {
                        format!("{name}: Deltas<DeltaProto<JsonStruct>>")
                    }
                    _ => panic!("Unknown mode"),
                },

                ModuleInput::Source { source } => "block: EthBlock".to_string(),
            }
        }

        fn generate_formatters(&self) -> Option<String> {
            match self {
                ModuleInput::Store { store: name, mode } => {
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

    pub fn generate_streamline_modules(modules: &Vec<&ModuleData>) -> String {
        modules.iter().fold(String::new(), |acc, e| {
            let module_code = match e.kind() {
                ModuleKind::Map => generate_mfn(e.name(), e.inputs(), e.handler()),
                ModuleKind::Store => {
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

    fn generate_mfn(name: &str, inputs: &Vec<ModuleInput>, handler: &str) -> String {
        // The rust fn inputs
        let module_inputs = inputs.generate();
        let formatters = if let Some(val) = inputs.generate_formatters() {
            val
        } else {
            String::new()
        };

        let arg_names = inputs.iter().map(|input| input.name()).collect::<Vec<_>>();

        let args = if arg_names.len() == 1 {
            format!("{},", arg_names[0])
        } else {
            arg_names.join(",")
        };

        format!(
            r#"
#[substreams::handlers::map]
fn {name}({module_inputs}) -> Option<JsonStruct> {{
    {formatters}
    let (mut engine, mut scope) = engine_init!();
    register_builtins(&mut engine);
    let ast = engine.compile(RHAI_SCRIPT).unwrap();
    let result: Dynamic = engine.call_fn(&mut scope, &ast, "{handler}", ({args})).expect("Call failed");
    let mut output_map = Map::new();
    if result.is_unit() {{
        None
    }} else {{
        let result = serde_json::to_value(&result).expect("Couldn't convert from Dynamic!");
        output_map.insert("result".to_string(), result);
        Some(serde_json::from_value(output_map.into()).expect("Failed to convert output_map to json"))
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
        let arg_names = inputs.iter().map(|input| input.name()).collect::<Vec<_>>();

        let args = inputs
            .iter()
            .map(|input| input.name())
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            r#"
#[substreams::handlers::store]
fn {name}({module_inputs}, streamline_store_param: {store_kind}) {{
    let streamline_store_param = Rc::new(streamline_store_param);
    {formatters}
    let (mut engine, mut scope) = engine_init!();
    register_builtins(&mut engine);
    let ast = engine.compile(RHAI_SCRIPT).unwrap();
    let result:Dynamic = engine.call_fn(&mut scope, &ast, "{handler}", ({args}, streamline_store_param)).expect("Call failed");
}}
    "#
        )
    }
}

pub mod yaml {
    use crate::packages::streamline::modules::{ModuleDag, ModuleKind};

    const TEMPLATE_YAML: &'static str = "
specVersion: v0.1.0
package:
  name: erc721
  version: v0.1.0

imports:
  sql: https://github.com/streamingfast/substreams-sink-sql/releases/download/protodefs-v1.0.2/substreams-sink-sql-protodefs-v1.0.2.spkg
  database_change: https://github.com/streamingfast/substreams-sink-database-changes/releases/download/v1.2.1/substreams-database-change-v1.2.1.spkg

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

    pub fn generate_yaml(modules: &ModuleDag) -> String {
        let modules = modules
            .modules
            .iter()
            .filter_map(|(_, module)| match module.kind() {
                ModuleKind::Map => Some(module),
                ModuleKind::Store => Some(module),
                ModuleKind::Source => None,
            })
            .collect::<Vec<_>>();

        let yaml = serde_yaml::to_string(&modules).unwrap();

        TEMPLATE_YAML.replace("$$MODULES$$", &yaml)
    }
}
