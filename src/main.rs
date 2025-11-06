mod frontend;
mod middle;

use frontend::{lexer, parser};
use middle::lower;

fn main() {
    let tests = [
        "1 + 2 * 3",
        "-(4 - 2)",
        "foo(1, 2 + x)",
        "a + b * c + d",
    ];

    for t in tests {
        println!("==== {t} ====");
        let tokens = lexer::lex(t);

        match parser::parse_tokens(tokens) {
            Ok(expr) => {
                println!("AST: {expr}");

                let module = lower::lower_expr_to_module(&expr);

                println!("IR: {:#?}", module);
            }
            Err(e) => eprintln!("Parse error for '{t}': {e}"),
        }
    }
}
