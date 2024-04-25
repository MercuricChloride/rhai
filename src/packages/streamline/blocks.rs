use crate::{plugin::*, Scope};

#[export_module]
pub mod blocks {
    use substreams::{pb::substreams::Clock as SubstreamsClock, Hex};
    use substreams_ethereum::pb::eth::v2::{Block, Log};

    pub type EthBlock = Block;
    pub type Clock = SubstreamsClock;

    #[rhai_fn(get = "logs", pure)]
    pub fn logs(block: &mut EthBlock) -> Vec<Log> {
        block.logs().map(|log| log.log.clone()).collect()
    }

    #[rhai_fn(get = "number", pure)]
    pub fn number(block: &mut EthBlock) -> String {
        block.number.to_string()
    }

    #[rhai_fn(get = "hash", pure)]
    pub fn hash(block: &mut EthBlock) -> String {
        format!("0x{}", Hex(&block.hash))
    }

    #[rhai_fn(get = "timestamp", pure)]
    pub fn timestamp(block: &mut EthBlock) -> String {
        block.timestamp_seconds().to_string()
    }

    #[rhai_fn(get = "timestamp", pure)]
    pub fn clock_timestamp(clock: &mut Clock) -> String {
        clock.timestamp.as_ref().unwrap().to_string()
    }

    #[rhai_fn(get = "number", pure)]
    pub fn clock_number(clock: &mut Clock) -> String {
        clock.number.to_string()
    }
}

pub fn init_globals(engine: &mut Engine, scope: &mut Scope) {
    engine.register_type::<blocks::EthBlock>();
}
