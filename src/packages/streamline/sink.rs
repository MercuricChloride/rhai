use std::{cell::RefCell, rc::Rc};

use std::collections::{BTreeMap, HashMap};

use crate::ImmutableString;

use super::modules::{Kind, Module};

macro_rules! impl_from {
    ($variant:ident) => {
        impl From<$variant> for ResolvedModule {
            fn from(value: $variant) -> Self {
                Self::$variant(value)
            }
        }
    };
}

pub trait ModuleResolver {
    ///Adds a new module config to the resolver. This will not overwrite an existing key.
    fn set_module_once(&mut self, module_name: ImmutableString, module: ResolvedModule);

    ///Adds a new module config to the resolver. This will overwrite an existing key.
    fn set_module(&mut self, module_name: ImmutableString, module: ResolvedModule);

    /// Returns an optional tuple containing:
    /// (ResolvedModule, Option<SinkConfig>)
    /// If the second item in the tuple is some, it represents the module being a sink
    /// and needs some massaging for the generated code
    fn get(&self, module_name: ImmutableString) -> Option<(&ResolvedModule, Option<&SinkConfig>)>;

    /// Returns a list of the modules a user has defined and included
    /// Note that this returns a Vec<Module> not resolved module.
    /// This is because the user cannot define a source block, or a sink config variant of resolved module.
    fn get_user_modules(&self) -> Vec<&Module>;

    /// A function to add a mfn, this is for compatability with the older API
    fn add_mfn(&mut self, name: ImmutableString, inputs: Vec<ImmutableString>) {
        self.set_module(
            name.clone(),
            ResolvedModule::Module(Module::new(name, &inputs, Kind::Map)),
        );
    }

    /// A function to add a sfn, this is for compatability with the older API
    fn add_sfn(&mut self, name: ImmutableString, inputs: Vec<ImmutableString>) {
        self.set_module(
            name.clone(),
            ResolvedModule::Module(Module::new(name, &inputs, Kind::Store)),
        );
    }
}

#[derive(Clone)]
pub enum ResolvedModule {
    Module(Module),
    SinkConfig(SinkConfig),
    Source(Source),
}

impl_from!(Module);
impl_from!(SinkConfig);
impl_from!(Source);

#[derive(Clone)]
pub struct Source {
    pub protobuf_name: ImmutableString,
    pub rust_name: ImmutableString,
}

impl Source {
    pub fn eth_block() -> Self {
        Self {
            protobuf_name: "sf.ethereum.type.v2.Block".into(),
            rust_name: "EthBlock".into(),
        }
    }
}

#[derive(Clone)]
pub struct SinkConfig {
    pub protobuf_name: String,
    pub rust_name: String,
    pub crate_name: String,
    pub fully_qualified_path: String,
    pub spkg_link: String,
}

impl SinkConfig {
    pub fn graph_out() -> Self {
        Self {
            protobuf_name: "substreams.entity.v1.EntityChanges".into(),
            rust_name: "substreams_entity_change::pb::entity::EntityChanges".into(),
            crate_name: "streamline_subgraph_conversions".into(),
            fully_qualified_path:
                "rhai::packages::streamline::graph_out::as_entity_changes(result)".into(),
            spkg_link: "HARDCODED_FOR_NOW".into(),
        }
    }
}

#[derive(Clone)]
pub struct DefaultModuleResolver {
    modules: HashMap<ImmutableString, ResolvedModule>,
    sinks: HashMap<ImmutableString, SinkConfig>,
}

impl DefaultModuleResolver {
    pub fn new() -> Self {
        let mut modules = HashMap::new();
        let mut sinks = HashMap::new();
        sinks.insert("graph_out".into(), SinkConfig::graph_out().into());
        modules.insert("BLOCK".into(), Source::eth_block().into());

        Self { modules, sinks }
    }

    pub fn new_shared() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::new()))
    }
}

impl Default for DefaultModuleResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleResolver for DefaultModuleResolver {
    fn set_module_once(&mut self, module_name: ImmutableString, module: ResolvedModule) {
        let contains = self.modules.contains_key(&module_name);
        if !contains {
            self.modules.insert(module_name, module);
        }
    }

    fn set_module(&mut self, module_name: ImmutableString, module: ResolvedModule) {
        self.modules.insert(module_name, module);
    }

    fn get(&self, module_name: ImmutableString) -> Option<(&ResolvedModule, Option<&SinkConfig>)> {
        let sink = self.sinks.get(&module_name);
        let module = self.modules.get(&module_name);
        if let Some(module) = module {
            Some((module, sink))
        } else {
            None
        }
    }

    fn get_user_modules(&self) -> Vec<&Module> {
        self.modules
            .values()
            .filter_map(|e| match e {
                ResolvedModule::Module(module) => Some(module),
                _ => None,
            })
            .collect::<Vec<_>>()
    }
}
