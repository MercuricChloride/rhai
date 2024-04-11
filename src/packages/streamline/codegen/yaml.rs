use std::collections::HashMap;

use serde_yaml::Value;

use crate::{
    packages::streamline::{
        constants::TEMPLATE_YAML,
        modules::{Accessor, Input, Kind},
        sink::{ModuleResolver, ResolvedModule},
    },
    ImmutableString,
};

use super::Codegen;

/// The yaml code generation struct
pub struct YamlGenerator(Box<dyn ModuleResolver>, Option<i64>);

impl YamlGenerator {
    /// Creates a new Yaml Code Generator
    pub fn new(resolver: Box<dyn ModuleResolver>, start_block: Option<i64>) -> Self {
        Self(resolver, start_block)
    }
}

impl Codegen for YamlGenerator {
    fn generate(&self) -> String {
        let Self(resolver, start_block) = &self;
        let modules = resolver.get_user_modules();
        let mut yaml_modules = vec![];

        for module in modules.iter() {
            let inputs = module
                .inputs
                .iter()
                .filter_map(|e| YamlInput::new(e, &resolver))
                .collect::<Vec<_>>();

            let yaml_module = YamlModule::new(module.name.clone(), &resolver, inputs);
            yaml_modules.push(yaml_module.to_yaml(*start_block));
        }

        let module_code = serde_yaml::to_string(&yaml_modules).unwrap();
        TEMPLATE_YAML
            .replace("$$MODULES$$", &module_code)
            .to_string()
    }
}

#[derive(Default, Clone)]
/// A representation of a module input, for the yaml output
pub struct YamlInput {
    name: ImmutableString,
    kind: ImmutableString,
    mode: Option<ImmutableString>,
}

/// A representation of a module, for the yaml output
pub struct YamlModule {
    name: ImmutableString,
    kind: ImmutableString,
    inputs: Vec<YamlInput>,
    update_policy: Option<ImmutableString>,
    output: ImmutableString,
}

impl YamlModule {
    /// Creates a new YamlModule
    pub fn new(
        name: ImmutableString,
        resolver: &Box<dyn ModuleResolver>,
        inputs: Vec<YamlInput>,
    ) -> Self {
        let (module, sink_config) = resolver
            .get(name.clone())
            .expect(&format!("No module found for: {}", &name));

        let mut yaml_module = match module {
            ResolvedModule::Module(module) => {
                if let Kind::Map = module.kind {
                    YamlModule {
                        name,
                        inputs,
                        kind: module.kind.to_string().into(),
                        update_policy: None,
                        output: module.output_type(),
                    }
                } else {
                    YamlModule {
                        name,
                        inputs,
                        kind: module.kind.to_string().into(),
                        update_policy: module.update_policy().map(|e| e.to_proto_string()),
                        output: module.output_type(),
                    }
                }
            }
            _ => unreachable!(),
        };

        if let Some(config) = sink_config {
            yaml_module.output = config.protobuf_name.as_str().into();
        }

        return yaml_module;
    }

    /// Converts a module into a serde_yaml::Value
    pub fn to_yaml(&self, start_block: Option<i64>) -> Value {
        let mut map: HashMap<ImmutableString, Value> = HashMap::new();
        if let Some(start_block) = start_block {
            map.insert("initialBlock".into(), Value::Number(start_block.into()));
        }
        map.insert("name".into(), Value::String(self.name.clone().into()));
        map.insert("kind".into(), Value::String(self.kind.clone().into()));
        let param_string_input = {
            let mut map = HashMap::new();
            map.insert("params", "string");
            let input = serde_yaml::to_value(map).expect("Failed to add params to input!");
            input
        };
        let mut inputs = vec![param_string_input];
        inputs.extend(
            self.inputs
                .clone()
                .iter()
                .map(|e| e.to_yaml())
                .collect::<Vec<_>>(),
        );
        map.insert("inputs".into(), serde_yaml::to_value(inputs).unwrap());

        if let Some(update_policy) = &self.update_policy {
            map.insert(
                "valueType".into(),
                Value::String(self.output.clone().into()),
            );
            map.insert("updatePolicy".into(), Value::String(update_policy.into()));
        } else {
            let mut output_map: HashMap<ImmutableString, Value> = HashMap::new();
            output_map.insert("type".into(), Value::String(self.output.clone().into()));
            map.insert("output".into(), serde_yaml::to_value(output_map).unwrap());
        }

        serde_yaml::to_value(&map).unwrap()
    }
}

impl YamlInput {
    /// Creates a new YamlInput
    pub fn new(input: &Input, resolver: &Box<dyn ModuleResolver>) -> Option<Self> {
        let name = input.name.split(":").collect::<Vec<_>>()[0];
        if let Accessor::Store(_) = input.access {
            return None;
        }

        let (input_module, _) = resolver.get(name.into()).expect(&format!(
            "Tried to use module as input, but module isn't defined! {:?}",
            name
        ));

        Some(match input_module {
            ResolvedModule::Module(module) => {
                let is_store = module.kind == Kind::Store;
                let access_mode: Option<ImmutableString> = if is_store {
                    match input.access {
                        Accessor::Deltas => Some("deltas".into()),
                        Accessor::Get | Accessor::Default => Some("get".into()),
                        Accessor::Store(_) => unreachable!(),
                    }
                } else {
                    None
                };

                Self {
                    name: module.name.clone(),
                    kind: module.kind.to_string().into(),
                    mode: access_mode,
                }
            }
            ResolvedModule::SinkConfig(_) => unreachable!(),
            ResolvedModule::Source(source) => Self {
                name: source.protobuf_name.as_str().into(),
                kind: "source".into(),
                ..Default::default()
            },
        })
    }

    /// Converts the YamlInput, into a serde_yaml::Value
    pub fn to_yaml(&self) -> Value {
        let mut map: HashMap<ImmutableString, Value> = HashMap::new();

        map.insert(self.kind.clone(), Value::String(self.name.as_str().into()));
        if let Some(mode) = &self.mode {
            map.insert("mode".into(), Value::String(mode.into()));
        }

        serde_yaml::to_value(&map).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::packages::streamline::sink::DefaultModuleResolver;

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

        let generator = YamlGenerator::new(resolver, None);

        let source = generator.generate();

        println!("{source}");
    }
}
