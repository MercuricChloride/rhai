use super::Codegen;
use crate::packages::streamline;
use crate::packages::streamline::constants::{
    MFN_ATTRIBUTE, MFN_DEFAULT_CONVERSION, MFN_OUTPUT, MFN_OUTPUT_TYPE, SFN_ADD, SFN_ATTRIBUTE,
    SFN_BIGINT_DELTAS, SFN_BIGINT_GET, SFN_JSON_DELTAS, SFN_JSON_GET, SFN_SET, SFN_SET_ONCE,
};
use crate::packages::streamline::modules::{Accessor, Input, Kind, UpdatePolicy};
use crate::ImmutableString;
use std::rc::Rc;
use streamline::modules as m; //::{Accessor, Kind, Input as SInput, Module as SModule};
use streamline::sink::{DefaultModuleResolver, ModuleResolver, ResolvedModule};

/// The rust code generation struct
pub struct RustGenerator(Box<dyn ModuleResolver>);

impl RustGenerator {
    /// Creates a new Rust Code Generator
    pub fn new(resolver: Box<dyn ModuleResolver>) -> Self {
        Self(resolver)
    }
}

impl Codegen for RustGenerator {
    fn generate(&self) -> String {
        let resolver = &self.0;
        let modules = resolver.get_user_modules();
        let mut output = String::new();

        for module in modules.iter() {
            let inputs = module
                .inputs
                .iter()
                .filter_map(|e| RustInput::new(e, &resolver))
                .map(|e| Box::new(e) as Box<dyn Codegen>)
                .collect::<Vec<_>>();

            let rust_handler = RustHandler::new(module.name.clone(), &resolver, inputs);
            output.push_str(&rust_handler.generate());
        }

        output
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
    pub fn new(input: &Input, resolver: &Box<dyn ModuleResolver>) -> Option<Self> {
        let Input { name, access } = &input;
        let name = name.split(":").collect::<Vec<_>>()[0];

        if let Accessor::Store(store) = input.access {
            let name = name.clone().into();
            let value_type = match store {
                UpdatePolicy::Add => SFN_ADD,
                UpdatePolicy::Set => SFN_SET,
                UpdatePolicy::SetOnce => SFN_SET_ONCE,
            }
            .into();
            return Some(RustInput { name, value_type });
        }

        let (resolved, sink_config) = resolver
            .get(name.into())
            .expect(&format!("No module found for: {}", &name));

        let mut input = match resolved {
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
            ResolvedModule::SinkConfig(sink) => unreachable!(),
            ResolvedModule::Source(source) => RustInput {
                name: name.into(),
                value_type: source.rust_name.as_str().into(),
            },
        };

        if let Some(config) = sink_config {
            input.value_type = config.rust_name.as_str().into();
        };

        return Some(input);
    }
}

impl RustHandler {
    pub fn new(
        name: ImmutableString,
        resolver: &Box<dyn ModuleResolver>,
        inputs: Vec<Box<dyn Codegen>>,
    ) -> Self {
        let (module, sink_config) = resolver
            .get(name.clone())
            .expect(&format!("No module found for: {}", &name));

        let mut handler = match module {
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
            ResolvedModule::SinkConfig(_) => unreachable!(),
            ResolvedModule::Source(_) => unreachable!(),
        };

        if let Some(config) = sink_config {
            handler.conversion = config.fully_qualified_path.as_str().into();
            let sink_output = config.rust_name.as_str().into();
            let new_output_type = handler
                .output_type
                .clone()
                .replace(MFN_OUTPUT_TYPE, sink_output);
            handler.output_type = new_output_type.into();
        }

        return handler;
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

        // returns (needs_formatting, name)
        let needs_formatting = |generated_input: &str| {
            let name = generated_input.split(":").collect::<Vec<_>>()[0];
            (
                generated_input.contains("StoreGet")
                    || generated_input.contains("Deltas")
                    || generated_input.contains("StoreSet")
                    || generated_input.contains("StoreAdd"),
                name.to_string(),
            )
        };

        // Store modules don't do anything with the result of the function call, so we set the 'body' to be an empty string
        // Otherwise we have to apply some conversions to them
        let mut body: String = "".into();

        let inputs = inputs.iter().map(|e| e.generate()).collect::<Vec<_>>();
        // We need to track if there is a single input to the module, so we can add the extra comma to the end of the tuple in the rust code
        // (foo) evaluates to foo
        // (foo,) is a single len tuple containing foo
        let single_input = inputs.len() == 1;

        let fn_inputs = &inputs.join(",");
        let formatters = inputs
            .clone()
            .iter()
            .filter_map(|input| {
                // TODO I need to make a more robust version of this as plugin features get added
                // but for now this is totally fine
                let (needs_formatting, name) = needs_formatting(&input);
                if needs_formatting {
                    return Some(format!("let {name} = Rc::new({name});"));
                }

                let name = input.split(":").collect::<Vec<_>>()[0];
                if !name.contains("BLOCK") {
                    return Some(format!("let {name} = to_dynamic({name}).unwrap();"));
                }

                None
            })
            .collect::<Vec<_>>()
            .join("\n");

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
fn {name}(substreams_param_string: String, {fn_inputs}) {output_type} {{
{formatters}
    let (mut engine, mut scope) = engine_init!();
    let substreams_param_string: serde_json::Value = serde_json::from_str(&substreams_param_string).unwrap_or_default();
    scope.push_constant("PARAMS", to_dynamic(substreams_param_string).expect("Couldn't convert (param string as Json) into a Dynamic Value!"));
    let ast = engine.compile(RHAI_SCRIPT).unwrap();
    let mut result: Dynamic = engine.call_fn(&mut scope, &ast, "{name}", ({handler_inputs})).expect("Call failed");
    {body}
}}
"#,
        )
    }
}

impl Codegen for RustInput {
    fn generate(&self) -> String {
        let Self { name, value_type } = &self;
        format!("{name}: {value_type}")
    }
}

#[cfg(test)]
mod tests {
    use self::streamline::modules::Module;

    use super::*;

    fn setup() -> Box<dyn ModuleResolver> {
        let mut resolver = DefaultModuleResolver::new();
        resolver.add_mfn("map_events".into(), vec!["BLOCK".into()]);
        resolver.add_mfn("graph_out".into(), vec!["map_events".into()]);
        Box::new(resolver) as Box<dyn ModuleResolver>
    }

    #[test]
    fn test_mfn_generation() {
        let resolver = setup();

        let generator = RustGenerator::new(resolver);

        let source = generator.generate();

        println!("{source}");
    }
}

// fn generate_formatters(&self) -> Option<String> {
//     match self {
//         ModuleInput::Store {
//             store: name, mode, ..
//         } => {
//             if mode.as_str() == "deltas" {
//                 return Some(format!("let {name} = Rc::new({name});"));
//             }

//             if mode.as_str() == "get" {
//                 return Some(format!("let {name} = Rc::new({name});"));
//             }
//         }
//         _ => {}
//     }
//     None
// }
