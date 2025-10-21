use logos::Logos;
use chumsky::prelude::*;

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f]+")] // Ignore this regex pattern between tokens

enum Token {
    // Operators / punctuation
    #[token("+")] Addition,
    #[token("-")] Subtraction,
    #[token("*")] Multiplication,
    #[token("/")] Division,
    #[token("(")] LeftParenthesis,
    #[token(")")] RightParenthesis,
    #[token(":")] Colon,
    #[token(",")] Comma,
    #[token("=")] Equals,

    // Keywords 
    #[token("def")] Def,
    #[token("for")] For,
    #[token("in")] In,
    #[token("return")] Return,

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().map_err(|_| ()))]
    Number(i64),

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Identifier(String),
}

#[derive(Debug, Clone)]
pub enum Expression {
    Number(i64),
    Identifier(String),
    Unary { op: UnOp, rhs: Box<Expression> }, //a = 5
    Binary { op: BinOp, lhs: Box<Expression>, rhs: Box<Expression> }, //a + b
}

#[derive(Debug, Clone, Copy)]
pub enum BinOp { Add, Sub, Mul, Div }

#[derive(Debug, Clone, Copy)]
pub enum UnOp { Pos, Neg }



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