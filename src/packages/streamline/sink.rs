use std::{cell::RefCell, rc::Rc};

use std::collections::BTreeMap;

#[derive(Clone)]
pub struct SinkConfig {
    pub protobuf_name: String,
    pub crate_name: String,
    pub spkg_link: String,
}

impl SinkConfig {
    pub fn graph_out() -> Self {
        Self {
            protobuf_name: "substreams.entity.v1.EntityChanges".into(),
            crate_name: "streamline_subgraph_conversions".into(),
            spkg_link: "HARDCODED_FOR_NOW".into(),
        }
    }
}

#[derive(Default, Clone)]
pub struct SinkConfigMap {
    /// A map from module_name -> Sink Config
    pub sinks: BTreeMap<String, SinkConfig>,
}

impl SinkConfigMap {
    pub fn new_shared() -> GlobalSinkConfig {
        let mut map = SinkConfigMap::default();
        map.sinks
            .insert("graph_out".into(), SinkConfig::graph_out());
        Rc::new(RefCell::new(SinkConfigMap::default()))
    }
}

pub type GlobalSinkConfig = Rc<RefCell<SinkConfigMap>>;
