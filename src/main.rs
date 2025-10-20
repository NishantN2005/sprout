use logos::Logos;

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f]+")] // Ignore this regex pattern between tokens
enum Token {
    #[token("+")]
    Addition,

    #[token("-")]
    Subtraction,

    #[token("*")]
    Multiplication,

    #[token("/")]
    Division,

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().map_err(|_| ()))]
    Number(i64),
}

fn main() {
    for token in Token::lexer("11 + 2 * 3") {
        println!("{:?}", token);
    }
}