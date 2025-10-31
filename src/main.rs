use std::fmt;

// Simple hand-rolled lexer tokens
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Comma,
    Number(i64),
    Ident(String),
    Eof,
}

fn is_ident_start(c: char) -> bool { c.is_ascii_alphabetic() || c == '_' }
fn is_ident_continue(c: char) -> bool { c.is_ascii_alphanumeric() || c == '_' }

fn lex(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' | '\r' => { chars.next(); }
            '+' => { chars.next(); tokens.push(Token::Plus); }
            '-' => { chars.next(); tokens.push(Token::Minus); }
            '*' => { chars.next(); tokens.push(Token::Star); }
            '/' => { chars.next(); tokens.push(Token::Slash); }
            '(' => { chars.next(); tokens.push(Token::LParen); }
            ')' => { chars.next(); tokens.push(Token::RParen); }
            ',' => { chars.next(); tokens.push(Token::Comma); }
            '0'..='9' => {
                let mut num = 0i64;
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() {
                        num = num * 10 + (d as i64 - '0' as i64);
                        chars.next();
                    } else { break; }
                }
                tokens.push(Token::Number(num));
            }
            _ if is_ident_start(c) => {
                let mut s = String::new();
                while let Some(&ch) = chars.peek() {
                    if is_ident_continue(ch) { s.push(ch); chars.next(); } else { break; }
                }
                tokens.push(Token::Ident(s));
            }
            _ => { chars.next(); }
        }
    }

    tokens.push(Token::Eof);
    tokens
}

// AST nodes
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(i64),
    Ident(String),
    Unary { op: UnaryOp, expr: Box<Expr> },
    Binary { left: Box<Expr>, op: BinaryOp, right: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp { Neg }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOp { Add, Sub, Mul, Div }

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Number(n) => write!(f, "{}", n),
            Expr::Ident(s) => write!(f, "{}", s),
            Expr::Unary { op, expr } => write!(f, "({:?} {})", op, expr),
            Expr::Binary { left, op, right } => write!(f, "({} {:?} {})", left, op, right),
            Expr::Call { callee, args } => {
                write!(f, "{}(", callee)?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", a)?;
                }
                write!(f, ")")
            }
        }
    }
}

// Simple recursive-descent parser with precedence climbing
struct Parser { tokens: Vec<Token>, pos: usize }

impl Parser {
    fn new(tokens: Vec<Token>) -> Self { Self { tokens, pos: 0 } }
    fn peek(&self) -> &Token { &self.tokens[self.pos] }
    fn next(&mut self) -> &Token { if self.pos < self.tokens.len()-1 { self.pos += 1; } &self.tokens[self.pos] }

    fn parse_expression(&mut self) -> Result<Expr, String> { self.parse_prec(0) }

    fn precedence(tok: &Token) -> Option<(u8, bool)> {
        match tok {
            Token::Plus => Some((1, true)),
            Token::Minus => Some((1, true)),
            Token::Star => Some((2, true)),
            Token::Slash => Some((2, true)),
            _ => None,
        }
    }

    fn parse_prec(&mut self, min_prec: u8) -> Result<Expr, String> {
        let mut left = match self.peek() {
            Token::Minus => { self.next(); let rhs = self.parse_prec(3)?; Expr::Unary { op: UnaryOp::Neg, expr: Box::new(rhs) } }
            Token::Number(n) => { let v = *n; self.next(); Expr::Number(v) }
            Token::Ident(s) => {
                let name = s.clone(); self.next();
                if let Token::LParen = self.peek() {
                    self.next();
                    let mut args = Vec::new();
                    if let Token::RParen = self.peek() { self.next(); Expr::Call { callee: Box::new(Expr::Ident(name)), args } }
                    else {
                        loop {
                            let e = self.parse_expression()?;
                            args.push(e);
                            match self.peek() {
                                Token::Comma => { self.next(); continue; }
                                Token::RParen => { self.next(); break; }
                                t => return Err(format!("Unexpected token in call args: {:?}", t)),
                            }
                        }
                        Expr::Call { callee: Box::new(Expr::Ident(name)), args }
                    }
                } else { Expr::Ident(name) }
            }
            Token::LParen => { self.next(); let e = self.parse_expression()?; if let Token::RParen = self.peek() { self.next(); e } else { return Err("Expected ')'".to_string()); } }
            t => return Err(format!("Unexpected token: {:?}", t)),
        };

        loop {
            let op_tok = self.peek().clone();
            if let Some((prec, left_assoc)) = Parser::precedence(&op_tok) {
                if prec < min_prec { break; }
                let _ = self.next();
                let next_min = if left_assoc { prec + 1 } else { prec };
                let right = self.parse_prec(next_min)?;
                let op = match op_tok {
                    Token::Plus => BinaryOp::Add,
                    Token::Minus => BinaryOp::Sub,
                    Token::Star => BinaryOp::Mul,
                    Token::Slash => BinaryOp::Div,
                    _ => unreachable!(),
                };
                left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
            } else { break; }
        }

        Ok(left)
    }
}

fn parse_input(src: &str) -> Result<Expr, String> {
    let tokens = lex(src);
    let mut p = Parser::new(tokens);
    let expr = p.parse_expression()?;
    match p.peek() { Token::Eof => Ok(expr), t => Err(format!("Unexpected trailing token: {:?}", t)), }
}

fn main() {
    let tests = ["1 + 2 * 3", "-(4 - 2)", "foo(1, 2 + x)", "a + b * c + d"];
    for t in tests {
        match parse_input(t) {
            Ok(ast) => println!("{} -> {}", t, ast),
            Err(e) => println!("Parse error for '{}': {}", t, e),
        }
    }
}