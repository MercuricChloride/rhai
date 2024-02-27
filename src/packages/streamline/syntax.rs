use std::fs;

use crate::{
    parser::ParseResult, Dynamic, Engine, EvalAltResult, EvalContext, Expression, ImmutableString,
};

/*
 * The syntax we are testing is:
 * `mfn epicNumber(block) { block.number }`
 *
 * So what this means is inputs[0] if mfn
 * inputs[1] is the name
 * inputs[2..len-2] are the arguments
 * inputs[len-1] is the body,
 */

fn parse_mfn(
    symbols: &[ImmutableString],
    look_ahead: &str,
    state: &mut Dynamic,
) -> ParseResult<Option<ImmutableString>> {
    if state.is_unit() {
        let map = crate::Map::new();
        *state = Dynamic::from_map(map);
    }

    let state: &mut crate::Map = state.downcast_mut().unwrap();

    let should_parse_body = state.get("should_parse_body");
    let done_parsing = state.get("done_parsing");

    // mfn [[NAME]]
    if symbols.len() == 1 {
        return ParseResult::Ok(Some("$ident$".into()));
    }

    if symbols.len() == 2 && look_ahead == "(" {
        return ParseResult::Ok(Some("$symbol$".into()));
    }

    if symbols.len() > 2 && should_parse_body.is_none() {
        if look_ahead == ")" {
            state.insert("should_parse_body".into(), true.into());
            ParseResult::Ok(Some("$symbol$".into()))
        } else if look_ahead == "," {
            ParseResult::Ok(Some("$symbol$".into()))
            //return ParseResult::Ok(Some("$symbol$".into()));
        } else {
            ParseResult::Ok(Some("$ident$".into()))
        }
    } else if done_parsing.is_none() {
        state.insert("done_parsing".into(), true.into());
        ParseResult::Ok(Some("$block$".into()))
    } else {
        ParseResult::Ok(None)
    }
}

fn impl_mfn(
    context: &mut EvalContext,
    inputs: &[Expression],
    state: &Dynamic,
) -> Result<Dynamic, Box<EvalAltResult>> {
    println!("impl_mfn: {:?}", inputs);
    println!("impl_mfn: {:?}", state);
    Ok(Dynamic::UNIT)
}

/// Allows for definition of modules similar to the fn keyword, but with a different syntax
pub fn register_mfn_syntax(engine: &mut Engine) {
    engine.register_custom_syntax_with_state_raw("mfn", parse_mfn, false, impl_mfn);
}

mod test {
    use super::*;
    use crate::{Engine, EvalAltResult, Scope};

    const test_code: &'static str = r#"
mfn add(x, y) {
 x + y
}
"#;

    #[test]
    fn test_register_mfn_syntax() {
        let mut engine = Engine::new();
        register_mfn_syntax(&mut engine);
        engine.eval::<Dynamic>(test_code).unwrap();
    }
}
