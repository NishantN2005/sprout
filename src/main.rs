mod frontend;
mod middle;
mod backend;

use frontend::{lexer, parser};
use middle::lower;
use backend::llvm;

fn main() {
    let tests = [
        "1 + 2 * 3",
        "-(4 - 2)",
        // "foo(1, 2 + x)", // this will probably fail until you handle vars
        // "a + b * c + d",
    ];

    for t in tests {
        println!("=== {t} ===");

        let tokens = lexer::lex(t);

        match parser::parse_tokens(tokens) {
            Ok(expr) => {
                println!("AST: {expr}");

                let ir_module = lower::lower_expr_to_module(&expr);
                println!("IR: {:#?}", ir_module);

                match llvm::jit_run_main(&ir_module) {
                    Ok(result) => println!("Result from JIT: {result}"),
                    Err(e) => eprintln!("Codegen/JIT error: {e}"),
                }
            }
            Err(e) => eprintln!("Parse error for '{t}': {e}"),
        }
    }
}
