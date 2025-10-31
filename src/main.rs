mod frontend;

fn main() {
    let tests = [
        "1 + 2 * 3",
        "-(4 - 2)",
        "foo(1, 2 + x)",
        "a + b * c + d",
    ];

    for t in tests {
        // 1) lex
        let tokens = frontend::lexer::lex(t);

        // 2) parse
        match frontend::parser::parse_tokens(tokens) {
            Ok(expr) => println!("{t} -> {expr}"),
            Err(e) => eprintln!("Parse error for '{t}': {e}"),
        }
    }
}
