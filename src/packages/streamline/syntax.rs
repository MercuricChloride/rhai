use core::ops::Deref;
use std::fs;

use smallvec::SmallVec;

use crate::{
    parser::ParseResult, Dynamic, Engine, EvalAltResult, EvalContext, Expr, Expression, FnAccess,
    FnArgsVec, FuncRegistration, ImmutableString, RhaiFunc, ScriptFuncDef,
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
    let engine = context.engine();
    let scope = context.scope_mut();
    let fn_name = inputs[0].get_string_value().unwrap();
    let args = inputs[2..inputs.len() - 2]
        .iter()
        .map(|input| input.get_string_value().unwrap().into())
        .collect::<Vec<ImmutableString>>();
    let args = FnArgsVec::from_vec(args);
    let body = inputs.last().unwrap();
    let stmt_block = match body.deref() {
        Expr::Stmt(s) => s.clone(),
        _ => panic!("Expected a block"),
    };

    let func = format!(
        r#"
    fn {fn_name}({args}) {{
        "todo"
    }}
"#,
        args = args.join(""),
        //stmt_block = stmt_block.statements().join("\n")
    );

    println!("{}", func);

    let ast = engine.compile(&func).unwrap();

    let result = engine.eval_ast_with_scope::<Dynamic>(scope, &ast).unwrap();

    Ok(result)
}

/// Allows for definition of modules similar to the fn keyword, but with a different syntax
pub fn register_mfn_syntax(engine: &mut Engine) {
    engine.register_custom_operator("mfn", 255).unwrap();
    engine.register_custom_syntax_with_state_raw("mfn", parse_mfn, true, impl_mfn);
}

mod test {
    use super::*;
    use crate::{Engine, EvalAltResult, Scope, AST};

    const test_code: &'static str = r#"
mfn add(x, y) {
 x + y
}

add(1,2)
"#;

    const other_test: &'static str = r#"
fn add(x, y) {
 x + y
}
"#;

    #[test]
    fn test_register_mfn_syntax() {
        let mut engine = Engine::new();
        let mut scope = Scope::new();
        let mut main_ast = AST::empty();
        register_mfn_syntax(&mut engine);
        let ast: Dynamic = engine.eval(test_code).unwrap();
        println!("{:?}", ast);

        //main_ast = main_ast.merge(&ast);

        let result = engine
            .call_fn::<Dynamic>(&mut scope, &main_ast, "add", (12, 12))
            .unwrap();
        println!("{}", result);
    }
}
