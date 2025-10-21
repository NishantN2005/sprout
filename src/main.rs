use logos::Logos;
use chumsky::prelude::*;

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

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Identifier(String),

    #[token("def")]
    Def,

    #[token("(")]
    LeftParenthesis,

    #[token(")")]
    RightParenthesis,

    #[token(":")]
    Colon,

    #[token(",")]
    Comma,
    
    #[token("for")]
    For,

    #[token("in")]
    In,

    #[token("=")]
    Equals,

    #[token("return")]
    Return,
}

fn main() {
    for token in Token::lexer("11 + 2 * 3") {
        println!("{:?}", token);
    }
    println!("----Test 1----");
    for token in Token::lexer("a = 1 + 1") {
        println!("{:?}", token);
    }
    println!("----Test 2----");
    for token in Token::lexer("def add(x, y): return x + y") {
        println!("{:?}", token);
    }
    println!("----Test 3----");
}