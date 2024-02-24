use std::fs;

use crate::{parser::ParseResult, Dynamic, Engine, ImmutableString};

fn parse_mfn_syntax(symbols: &[ImmutableString], look_ahead: &str, state: &mut Dynamic) -> ParseResult<Option<ImmutableString>> {
    println!("parse_mfn_syntax: {:?}", symbols);
    ParseResult::Ok(None)
    // match symbols.len() {
    //     1 => Ok(Some(symbols[0].clone())),
    //     _ => Err("Invalid syntax for mfn".into())
    // }
}

/// Allows for definition of modules similar to the fn keyword, but with a different syntax
pub fn register_mfn_syntax(engine: &mut Engine) {
    engine.register_custom_syntax_with_state_raw(
        "mfn",
        parse_mfn_syntax,
        true, 
        |context, inputs, state|{
            todo!()
        });
}

/// the syntax we are testing is `custom {}`
pub fn test_custom_syntax(engine: &mut Engine) {
    engine.register_custom_syntax_with_state_raw(
        "custom",
        |symbols, look_ahead, state| {
            todo!()
        }, false, 
        |context, inputs, state| {
            todo!()
        })
}

fn register_custom_syntax(engine: &mut Engine) {
    todo!()
}

mod test {
    use super::*;
    use crate::{Engine, EvalAltResult, Scope};

    #[test]
    fn test_register_mfn_syntax() {
        let mut engine = Engine::new();
        register_mfn_syntax(&mut engine);
        let result = engine.eval::<i64>("mfn add(x, y) { x + y }").unwrap();
        assert_eq!(result, 3);
    }
}
