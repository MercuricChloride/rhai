use crate::{plugin::*, Scope};
use core::cell::RefCell;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::{collections::BTreeMap, fs};

#[derive(Serialize, Deserialize, Clone)]
/// One of the two kinds of contract sources we support
pub enum ContractSource {
    Abi(String),
    Source(String),
    // TODO - add a way to load from an address
}

impl ContractSource {
    pub fn generate_source(&self, name: &str) -> String {
        match self {
            ContractSource::Abi(abi) => {
                format!("sol!({name}, r#\"{abi}\"#);")
            }
            ContractSource::Source(source) => {
                format!("sol!r#\"{source}\"#;")
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
/// A struct to hold the contracts that are imported into the runtime
pub struct ContractImports {
    // A map from the name of the contract, to the contract abi or source
    pub contracts: BTreeMap<String, ContractSource>,
}

impl ContractImports {
    pub fn new() -> Self {
        Self {
            contracts: BTreeMap::new(),
        }
    }

    /// A function to import an abi from a file
    pub fn add_abi(&mut self, name: String, abi_path: String) {
        let is_url = |path: &str| path.starts_with("http://") || path.starts_with("https://");

        let file = if is_url(&abi_path) {
            // TODO - download the abi from the url
            #[cfg(not(feature = "substreams_runtime"))]
            {
                let response = reqwest::blocking::get(abi_path).expect("GET request failed!");
                let content = response
                    .text()
                    .expect("Couldn't read the text of the response!");
                content
            }

            #[cfg(feature = "substreams_runtime")]
            {
                panic!("Can't download from a url from within the substreams runtime!")
            }
        } else {
            std::fs::read_to_string(&abi_path).expect("Couldn't read the file")
        };
        self.contracts.insert(name, ContractSource::Abi(file));
    }

    pub fn add_source(&mut self, name: String, source_path: String) {
        let file = std::fs::read_to_string(&source_path).expect("Couldn't read the file");
        self.contracts.insert(name, ContractSource::Source(file));
    }

    pub fn remove(&mut self, name: String) {
        self.contracts.remove(&name);
    }

    pub fn generate_sources(&self) -> String {
        let mut output = String::new();

        for (name, source) in &self.contracts {
            output.push_str(&source.generate_source(name));
        }

        output
    }
}

pub type GlobalContracts = Rc<RefCell<ContractImports>>;

pub struct AbiLookup {
    pub contracts: &'static GlobalContracts,
}

#[export_module]
pub mod abi_api {
    pub type Contracts = GlobalContracts;
}

pub fn init_globals(engine: &mut Engine, scope: &mut Scope) {
    let contract_imports = GlobalContracts::new(RefCell::new(ContractImports::new()));

    // Register a global variable for the contracts
    let contracts = contract_imports.clone();
    scope.push_constant("CONTRACTS", contracts);

    // add an import_source fn
    let contracts = contract_imports.clone();
    engine.register_fn("import_source", move |name: String, path: String| {
        (*contracts).borrow_mut().add_source(name, path);
    });

    // add an import_abi fn
    let contracts = contract_imports.clone();
    engine.register_fn("import_abi", move |name: String, path: String| {
        (*contracts).borrow_mut().add_abi(name, path);
    });

    // add a remove_contract fn
    let contracts = contract_imports.clone();
    engine.register_fn("remove_contract", move |name: String| {
        (*contracts).borrow_mut().remove(name);
    });

    let contracts = contract_imports.clone();
    engine.register_fn("contracts_source", move || {
        let contracts_source = (*contracts).borrow().generate_sources();
        #[cfg(feature = "dev")]
        fs::write("/tmp/contracts.rs", &contracts_source).unwrap();
        contracts_source
    });
}
