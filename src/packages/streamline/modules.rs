use serde::{Deserialize, Serialize};

use crate::packages::streamline::codegen::yaml::YamlGenerator;
use crate::serde::from_dynamic;
use crate::{plugin::*, tokenizer::Token, Array, Scope};
use core::cell::RefCell;
use core::str::FromStr;
use std::collections::BTreeMap;
use std::fs;
use std::rc::Rc;

use super::codegen::rust::RustGenerator;
use super::codegen::{self, Codegen};
use super::constants::{BIGINT_PROTO, INITIAL_BLOCK, JSON_VALUE_PROTO, MFN_OUTPUT_TYPE};
use super::module_types::ModuleData;
use super::sink::{DefaultModuleResolver, ModuleResolver};
//use super::sink::{GlobalSinkConfig, SinkConfigMap};

//converts an iterator of type T, into another type via the conversion
macro_rules! map_cast {
    ($coll:ident, $conversion:expr) => {
        $coll.iter().filter_map($conversion).collect()
    };
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum UpdatePolicy {
    Add,
    Set,
    SetOnce,
}

impl UpdatePolicy {
    /// Returns the protobuf type for the update policy
    pub fn to_proto_string(&self) -> ImmutableString {
        match &self {
            UpdatePolicy::Add => "add",
            UpdatePolicy::Set => "set",
            UpdatePolicy::SetOnce => "set_if_not_exists",
        }
        .into()
    }
}

#[derive(Default, Copy, Clone, Serialize, Deserialize)]
pub enum Accessor {
    /// foo:deltas
    Deltas,
    /// foo:get
    Get,
    /// s:add
    Store(UpdatePolicy),
    /// No accessor
    #[default]
    Default,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Input {
    pub name: String,
    pub access: Accessor,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum Kind {
    Map,
    Store,
}

impl ToString for Kind {
    fn to_string(&self) -> String {
        match &self {
            Kind::Map => "map",
            Kind::Store => "store",
        }
        .into()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Module {
    pub name: ImmutableString,
    pub inputs: Vec<Input>,
    pub kind: Kind,
}

impl Module {
    pub fn input_names(&self) -> Vec<&str> {
        self.inputs.iter().map(|e| e.name.as_str()).collect()
    }

    pub fn new(name: ImmutableString, inputs: &Vec<ImmutableString>, kind: Kind) -> Self {
        Self {
            name,
            inputs: map_cast!(inputs, |e| e.parse().ok()),
            kind,
        }
    }

    pub fn update_policy(&self) -> Option<UpdatePolicy> {
        if let Some(Accessor::Store(policy)) = &self.inputs.last().map(|i| i.access) {
            return Some(*policy);
        }

        None
    }

    pub fn output_type(&self) -> ImmutableString {
        match &self.update_policy() {
            Some(policy) => match policy {
                UpdatePolicy::Add => BIGINT_PROTO,
                _ => JSON_VALUE_PROTO,
            },
            // If none, this means it's a mfn, which means it outputs a JsonValue
            None => JSON_VALUE_PROTO,
        }
        .into()
    }
}

impl FromStr for Input {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let access = match s {
            s if s.ends_with(":deltas") => Accessor::Deltas,
            s if s.ends_with(":get") => Accessor::Get,
            s if s.ends_with(":add") => Accessor::Store(UpdatePolicy::Add),
            s if s.ends_with(":set") => Accessor::Store(UpdatePolicy::Set),
            s if s.ends_with(":setOnce") => Accessor::Store(UpdatePolicy::SetOnce),
            _ => Accessor::Default,
        };

        Ok(Input {
            name: s.to_string(),
            access,
        })
    }
}

pub fn init_globals(engine: &mut Engine, scope: &mut Scope) {
    let resolver = DefaultModuleResolver::new_shared();

    let modules = resolver.clone();
    engine.register_fn("add_mfn", move |name: ImmutableString, inputs: Array| {
        let inputs = map_cast!(inputs, |e| e.clone().into_immutable_string().ok());
        (*modules).borrow_mut().add_mfn(name, inputs);
        "Added mfn to DAG!".to_string()
    });

    let modules = resolver.clone();
    engine.register_fn("add_sfn", move |name: ImmutableString, inputs: Array| {
        let inputs = map_cast!(inputs, |e| e.clone().into_immutable_string().ok());
        (*modules).borrow_mut().add_sfn(name, inputs);
        "Added sfn to DAG!".to_string()
    });

    let modules = resolver.clone();
    engine.register_fn("generate_yaml", move |path: String| {
        let modules = (*modules).clone().borrow().clone();
        let generator = YamlGenerator::new(Box::new(modules) as Box<dyn ModuleResolver>);
        let yaml = generator.generate();
        fs::write(&path, yaml).unwrap();
        format!("Wrote yaml to {} successfully!", &path)
    });

    let modules = resolver.clone();
    engine.register_fn("generate_rust", move |path: String| {
        // we only call generate_rust once, so this is fine
        // we are just cloning all the module data underneath to use in the generation
        // and it should be the valid module data, since the variable we capture in the closure is a Rc<RefCell>
        // which points to the original module data
        let modules = (*modules).clone().borrow().clone();
        let generator = RustGenerator::new(Box::new(modules) as Box<dyn ModuleResolver>);
        fs::write(&path, generator.generate()).unwrap();
        format!("Wrote rust source to {} successfully!", &path)
    });

    // we use the substreams_runtime feature only when we are running in the substreams, not in the repl
    engine.register_fn("in_repl", move || {
        if cfg!(feature = "substreams_runtime") {
            false
        } else {
            true
        }
    });
}
