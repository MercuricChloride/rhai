fn init_imports() {
   import_abi("erc721", "/tmp/erc721.json");
}

fn dag_config() {
   //add_sfn("bar", [#{kind: "store", name: "some_store", mode: "deltas"}], "storeHandler");
   add_mfn("map_events", [#{kind: "source"}], "map_events");
}

fn codegen() {
// invoke the config functions
   init_imports();
   dag_config();

// invoke the codegen functions
   modules_source();
   contracts_source();
}

fn map_events(block) {
   #{
      foo: "HELLO RHAI!",
   }
}