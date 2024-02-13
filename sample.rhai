add_sfn("bar", [#{kind: "store", name: "some_store", mode: "deltas"}]);
add_mfn("foo", [#{kind: "map", "name": "map_events"}]);
MODULES.modules;