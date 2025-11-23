mod frontend;
mod middle;
mod backend;

use frontend::{lexer, parser};
use middle::lower;
use backend::llvm;

//read sprout files
use std::fs;


fn main() {
    let file_path = "tests";
    let paths = fs::read_dir(file_path).unwrap();

    let mut tests = Vec::new();

    for path_result in paths{
        let entry = path_result.expect("Error reading directory entry");
        let path = entry.path();

        let contents = fs::read_to_string(&path);

        tests.push(contents);
    }

    println!("{:?}", tests);
    
     for t in tests{
        let t = t.expect("Error reading test file");
        
        println!("=== {t} ===");

        let tokens = lexer::lex(&t);

        match parser::parse_tokens(tokens) {
            Ok(expr) => {
                for exp in expr.iter(){
                    println!("AST: {exp}");
                }

                let mut ir_module = lower::lower_program_to_module(&expr);

                //optimize module
                middle::opt::optimize_module(&mut ir_module);

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
