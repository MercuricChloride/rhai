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

pub struct RustGenerator(Box<dyn ModuleResolver>);

impl RustGenerator {
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
                .map(|e| RustInput::new(e, &resolver))
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
    pub fn new(input: &Input, resolver: &Box<dyn ModuleResolver>) -> Self {
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
        resolver: &Box<dyn ModuleResolver>,
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

    fn default_module() -> Module {
        todo!()
    }

    fn setup() -> Box<dyn ModuleResolver> {
        let mut resolver = DefaultModuleResolver::new();
        let module = default_module();
        resolver.set_module("map_events".into(), ResolvedModule::Module(module));
        Box::new(resolver) as Box<dyn ModuleResolver>
    }

    #[test]
    fn test_mfn_generation() {}
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
