use crate::frontend::lexer::Token;
use crate::frontend::ast::{Expr, UnaryOp, BinaryOp};

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self { Self { tokens, pos: 0 } }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn next(&mut self) -> &Token {
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        &self.tokens[self.pos]
    }

    fn parse_expression(&mut self) -> Result<Expr, String> {
        self.parse_prec(0)
    }

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
        // prefix
        let mut left = match self.peek() {
            Token::Minus => {
                self.next();
                let rhs = self.parse_prec(3)?;
                Expr::Unary { op: UnaryOp::Neg, expr: Box::new(rhs) }
            }
            Token::Semicolon =>{
                self.next();
                self.parse_expression()?
            }
            Token::Number(n) => {
                let v = *n;
                self.next();
                Expr::Number(v)
            }
            Token::Ident(s) => {
                let name = s.clone();
                self.next();
                // call?
                if let Token::LParen = self.peek() {
                    self.next(); // consume '('
                    let mut args = Vec::new();
                    if let Token::RParen = self.peek() {
                        self.next();
                        Expr::Call { callee: Box::new(Expr::Ident(name)), args }
                    } else {
                        loop {
                            let e = self.parse_expression()?;
                            args.push(e);
                            match self.peek() {
                                Token::Comma => { self.next(); }
                                Token::RParen => { self.next(); break; }
                                t => return Err(format!("Unexpected token in call args: {:?}", t)),
                            }
                        }
                        Expr::Call { callee: Box::new(Expr::Ident(name)), args }
                    }
                } else if let Token::Equals = self.peek(){
                    println!("Ident parsed in else if: {}", name);
                    self.next();
                    let expr = self.parse_expression()?;
                    println!("Assignment expression parsed: {}", expr);
                    Expr::Binary{
                        left: Box::new(Expr::Ident(name)),
                        op: BinaryOp::Assign,
                        right: Box::new(expr),}
                }else {
                    println!("Ident parsed in else: {}", name);
                    Expr::Ident(name)
                }
            }
            Token::LParen => {
                self.next();
                let e = self.parse_expression()?;
                if let Token::RParen = self.peek() {
                    self.next();
                    e
                } else {
                    return Err("Expected ')'".to_string());
                }
            }
            t => return Err(format!("Unexpected token: {:?}", t)),
        };

        // infix / precedence climbing
        loop {
            let op_tok = self.peek().clone();
            if let Some((prec, left_assoc)) = Parser::precedence(&op_tok) {
                if prec < min_prec { break; }
                self.next(); // consume op
                let next_min = if left_assoc { prec + 1 } else { prec };
                let right = self.parse_prec(next_min)?;
                let op = match op_tok {
                    Token::Plus => BinaryOp::Add,
                    Token::Minus => BinaryOp::Sub,
                    Token::Star => BinaryOp::Mul,
                    Token::Slash => BinaryOp::Div,
                    _ => unreachable!(),
                };
                left = Expr::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }

        Ok(left)
    }
}

fn parse_statement(tokens: Vec<Token>) -> Result<Vec<Vec<Token>>, String> {
        let mut statements: Vec<Vec<Token>> = Vec::new();
        let mut tmp = Vec::new();
        for stmt in tokens{
            if stmt == Token::Semicolon{
                statements.push(tmp);
                tmp = Vec::new();
            } else{
                tmp.push(stmt);
            }
        };
        Ok(statements)
    }

// public entry:
pub fn parse_tokens(tokens: Vec<Token>) -> Result<Vec<Expr>, String> {
    let mut p_stat = parse_statement(tokens)?;
    let mut p = Vec::new();

    for stmt in p_stat.iter(){
        let mut parse = Parser::new(stmt.clone());
        p.push(parse.parse_expression()?);
    }

    //let expr = p_stat.parse_expression()?;
    for expr in p.iter(){
        println!("Parsed expression: {}", expr);
    }

    Ok(p)
    /*match p.peek() {
        Token::Eof => Ok(Vec<Expr>),
        t => Err(format!("Unexpected trailing token: {:?}", t)),
    }*/
}
