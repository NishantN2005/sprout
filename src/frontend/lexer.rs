#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    If,
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Comma,
    Equals,
    Gt,
    Lt,
    EqComp,
    Colon,
    Semicolon,
    Number(i64),
    Ident(String),
    Eof,
}

fn is_ident_start(c: char) -> bool { c.is_ascii_alphabetic() || c == '_' }
fn is_ident_continue(c: char) -> bool { c.is_ascii_alphanumeric() || c == '_' }

pub fn lex(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' | '\r' => { chars.next(); }
            '#' => {
                while let Some(&ch) = chars.peek(){
                    if ch == '\n' {
                        break;
                    } else {
                        chars.next();
                    }
                    }
                }
            ';' => {chars.next(); tokens.push(Token::Semicolon);}
            '=' => {
                chars.next(); 
                if let Some(&'=') = chars.peek() {
                    chars.next();
                    tokens.push(Token::EqComp);
                } else {
                    tokens.push(Token::Equals);
                }
            }
            ':' =>{ chars.next(); tokens.push(Token::Colon); }
            '>' => {chars.next(); tokens.push(Token::Gt);}
            '<' => {chars.next(); tokens.push(Token::Lt);}
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
                if s== "if"{
                    tokens.push(Token::If);
                }else{
                tokens.push(Token::Ident(s));
                }
            }
            _ => { chars.next(); }
        }
    }
    println!("Lexer output tokens: {:?}", tokens);
    tokens.push(Token::Eof);
    tokens
}
