use crate::{plugin::*, Scope};

#[export_module]
pub mod blocks {
    use substreams::Hex;
    use substreams_ethereum::pb::eth::v2::{Block, Log};

    pub type EthBlock = Block;

    #[rhai_fn(get = "logs", pure)]
    pub fn logs(block: &mut EthBlock) -> Vec<Log> {
        block.logs().map(|log| log.log.clone()).collect()
    }

    #[rhai_fn(get = "number", pure)]
    pub fn number(block: &mut EthBlock) -> Dynamic {
        block.number.to_string().into()
    }

    #[rhai_fn(get = "hash", pure)]
    pub fn hash(block: &mut EthBlock) -> String {
        format!("0x{}", Hex(&block.hash))
    }

    #[rhai_fn(get = "timestamp", pure)]
    pub fn timestamp(block: &mut EthBlock) -> Dynamic {
        block.timestamp_seconds().to_string().into()
    }
}

pub fn init_globals(engine: &mut Engine, scope: &mut Scope) {
    engine.register_type::<blocks::EthBlock>();
}
