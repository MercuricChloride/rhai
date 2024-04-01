use std::{cell::RefCell, rc::Rc};

use std::collections::{BTreeMap, HashMap};

use crate::ImmutableString;

use super::modules::Module;

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

    /// Returns either a ResolvedModuleConfig if the module should have it's generated code changed
    fn get(&self, module_name: ImmutableString) -> Option<&ResolvedModule>;
}

pub enum ResolvedModule {
    Module(Module),
    SinkConfig(SinkConfig),
    Source(Source),
}

impl_from!(Module);
impl_from!(SinkConfig);
impl_from!(Source);

pub struct Source {
    pub protobuf_name: ImmutableString,
    pub rust_type_name: ImmutableString,
}

impl Source {
    pub fn eth_block() -> Self {
        Self {
            protobuf_name: "sf.ethereum.type.v2.Block".into(),
            rust_type_name: "EthBlock".into(),
        }
    }
}

#[derive(Clone)]
pub struct SinkConfig {
    pub protobuf_name: String,
    pub crate_name: String,
    pub fully_qualified_path: String,
    pub spkg_link: String,
}

impl SinkConfig {
    pub fn graph_out() -> Self {
        Self {
            protobuf_name: "substreams.entity.v1.EntityChanges".into(),
            crate_name: "streamline_subgraph_conversions".into(),
            fully_qualified_path: "rhai::packages::streamline::graph_out::as_entity_changes".into(),
            spkg_link: "HARDCODED_FOR_NOW".into(),
        }
    }
}

pub struct DefaultModuleResolver {
    modules: HashMap<ImmutableString, ResolvedModule>,
}

impl DefaultModuleResolver {
    pub fn new() -> Self {
        let mut modules = HashMap::new();
        modules.insert("graph_out".into(), SinkConfig::graph_out().into());
        modules.insert("BLOCK".into(), Source::eth_block().into());

        Self { modules }
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

    fn get(&self, module_name: ImmutableString) -> Option<&ResolvedModule> {
        self.modules.get(&module_name)
    }
}
