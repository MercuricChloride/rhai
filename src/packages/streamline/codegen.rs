use super::modules::ModuleData;
use super::abi::ContractImports;


pub mod rust {
    use crate::packages::streamline::modules::{ModuleInput, ModuleKind};

    use super::*;

    pub fn generate_streamline_modules(modules: &Vec<&ModuleData>) -> String {
        let mut output = String::new();

        for module in modules {
            match module.kind() {
                ModuleKind::Map => {
                    output.push_str(&generate_mfn(module.name(), module.inputs(), module.handler()));
                }
                ModuleKind::Store => {
                    output.push_str(&generate_sfn(module.name(), module.inputs(), module.handler()));
                }
                _ => panic!("We should never be generating a module that isn't a map or store module.")
            }
        }

        output
    }

    pub fn generate_streamline_sources(sources: &ContractImports) -> String {
        sources.generate_sources()
    }

    fn generate_input_type(input: &ModuleInput) -> String {
        match input {
            ModuleInput::Map { map: name} => {
                format!("{name}: JsonStruct")
            }

            ModuleInput::Store { store: name, mode: mode } => {
                match mode.as_str() {
                    "get" => {
                        format!("{name}: StoreGetProto<JsonStruct>")
                    }
                    "deltas" => {
                        format!("{name}: Deltas<DeltaProto<JsonStruct>")
                    }
                    _ => panic!("Unknown mode")
                }
            }

            ModuleInput::Source { source } => {
                "block: EthBlock".to_string()
            }
        }
    }

    fn generate_mfn(name: &str, inputs: &Vec<ModuleInput>, handler: &str) -> String {
        // The rust fn inputs
        let module_inputs = 
            inputs
            .iter()
            .map(generate_input_type)
            .collect::<Vec<_>>()
            .join(", ");

        let args = if inputs.len() == 1 {
            format!("{},", inputs[0].name())
        } else {
            inputs
            .iter()
            .map(|input| input.name())
            .collect::<Vec<_>>()
            .join(", ")
        };

        format!(r#"
#[substreams::handlers::map]
fn {name}({module_inputs}) -> Option<JsonStruct> {{
    let (mut engine, mut scope) = engine_init!();
    let ast = engine.compile(RHAI_SCRIPT).unwrap();
    let result: Dynamic = engine.call_fn(&mut scope, &ast, "{handler}", ({args})).expect("Call failed");
    from_dynamic::<JsonStruct>(&result).ok()
}}
    "#)
    }

    fn generate_sfn(name: &str, inputs: &Vec<ModuleInput>, handler: &str) -> String {
        // The rust
        // The rust fn inputs
        let module_inputs = 
            inputs
            .iter()
            .map(generate_input_type)
            .collect::<Vec<_>>()
            .join(", ");

        let store_kind = "SetIfNotExistsProto<JsonStruct>";

        let args = inputs
            .iter()
            .map(|input| input.name())
            .collect::<Vec<_>>()
            .join(", ");

        format!(r#"
#[substreams::handlers::store]
fn {name}({module_inputs}, streamline_store_param: {store_kind}) {{
    let (mut engine, mut scope) = engine_init!();
    let ast = engine.compile(RHAI_SCRIPT).unwrap();
    let result: Dynamic = engine.call_fn(&mut scope, &ast, "{handler}", ({args}, streamline_store_param)).expect("Call failed");
    from_dynamic::<JsonStruct>(&result).unwrap();
}}
    "#)
    }

}
