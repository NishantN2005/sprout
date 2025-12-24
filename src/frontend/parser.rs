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

            Token::Gt => Some((0, true)),
            Token::Lt => Some((0, true)),
            Token::EqComp => Some((0, true)),
            _ => None,
        }
    }

    fn parse_prec(&mut self, min_prec: u8) -> Result<Expr, String> {
        // prefix
        let mut left = match self.peek() {
            Token::For =>{
                self.next(); //consume 'for'
                let var = if let Token::Ident(name) = self.peek(){ name.clone() }
                    else { return Err("Expected identifier after 'for'".to_string()); };
                self.next(); //consume ident
                if let Token::In = self.peek(){ self.next(); } else { return Err("Expected 'in' after for loop variable".to_string()); }
                let iter_expr = self.parse_expression()?;
                if let Token::Colon = self.peek(){ self.next(); } else { return Err("Expected ':' after for iterator".to_string()); }
                let body = self.parse_expression()?;
                Expr::For { var, iter: Box::new(iter_expr), body: Box::new(body) }
            }
            Token::Def => {
                // parse: def name(param, ...): body
                self.next(); // consume 'def'
                let name = if let Token::Ident(n) = self.peek() {
                    let s = n.clone(); self.next(); s
                } else {
                    return Err("Expected function name after 'def'".to_string());
                };
                if let Token::LParen = self.peek() { self.next(); } else { return Err("Expected '(' after function name".to_string()); }
                let mut params: Vec<String> = Vec::new();
                if let Token::RParen = self.peek() {
                    self.next(); // empty params
                } else {
                    loop {
                        if let Token::Ident(n) = self.peek() {
                            params.push(n.clone());
                            self.next();
                        } else {
                            return Err("Expected identifier in parameter list".to_string());
                        }
                        match self.peek() {
                            Token::Comma => { self.next(); continue; }
                            Token::RParen => { self.next(); break; }
                            t => return Err(format!("Unexpected token in params: {:?}", t)),
                        }
                    }
                }
                if let Token::Colon = self.peek() { self.next(); } else { return Err("Expected ':' after function signature".to_string()); }
                let body = self.parse_expression()?;
                Expr::Function { name, params, body: Box::new(body) }
            }
            Token::If =>{
                self.next();
                let cond = self.parse_expression()?;
                if let Token::Colon = self.peek(){
                    self.next();
                } else {
                    return Err("Expected ':' after if condition".to_string());
                }
                let body = self.parse_expression()?;
                // optional else / else if
                let mut else_branch: Option<Box<Expr>> = None;
                // skip any semicolons/newline separators between the then-body and an else
                while let Token::Semicolon = self.peek() { self.next(); }
                if let Token::Else = self.peek() {
                    self.next();
                    // else if: let the parser parse the following `if` expression as the else-branch
                    if let Token::If = self.peek() {
                        // parse the nested if expression (this will consume the `If` and its parts)
                        let nested = self.parse_prec(0)?;
                        else_branch = Some(Box::new(nested));
                    } else {
                        // plain else: allow optional separators before ':' then parse body
                        while let Token::Semicolon = self.peek() { self.next(); }
                        if let Token::Colon = self.peek() {
                            self.next();
                        } else {
                            return Err("Expected ':' after else".to_string());
                        }
                        let else_body = self.parse_expression()?;
                        else_branch = Some(Box::new(else_body));
                    }
                }

                Expr::If { cond: Box::new(cond), body: Box::new(body), else_branch }
            }
            Token::Minus => {
                self.next();
                let rhs = self.parse_prec(3)?;
                Expr::Unary { op: UnaryOp::Neg, expr: Box::new(rhs) }
            }
            Token::LBracket =>{
                self.next();
                let mut ele = Vec::new();
                if let Token::RBracket = self.peek(){
                    self.next();
                }else{
                    loop{
                        let e = self.parse_expression()?;
                        ele.push(e);
                        match self.peek(){
                            Token::Comma => { self.next(); }
                            Token::RBracket => { self.next(); break; }
                            t => return Err(format!("Unexpected token in list literal: {:?}", t)),
                        }
                    }
                }
                Expr::List(ele)
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

                    Token::Gt => BinaryOp::Greater,
                    Token::Lt => BinaryOp::Less,
                    Token::EqComp => BinaryOp::Equal,
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
        // Legacy function - keep for callers that still use it, but
        // prefer token-stream parsing in `parse_tokens` now.
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
        // If there are any remaining tokens after the loop (no trailing
        // semicolon), treat them as the final statement.
        if !tmp.is_empty() {
            statements.push(tmp);
        }
        Ok(statements)
    }

// public entry:
pub fn parse_tokens(tokens: Vec<Token>) -> Result<Vec<Expr>, String> {
    // Parse the whole token stream in a single parser instance so
    // expressions like `if ...: ...; else: ...; + 1` can bind the `if`
    // expression into surrounding infix operators without requiring
    // parentheses.
    let mut parser = Parser::new(tokens);
    let mut results: Vec<Expr> = Vec::new();

    loop {
        match parser.peek() {
            Token::Eof => break,
            Token::Semicolon => { parser.next(); continue; }
            _ => {
                // parse an expression; after parsing we may need to fold
                // an operator-led continuation from the next statement into
                // the current expression (e.g., a trailing `+ 1;` should
                // become `(prev_expr) + 1`). This lets `if` act like an
                // expression that composes with a following operator.
                let mut expr = parser.parse_expression()?;

                // if the parsed expression is a `def` function, it's a top-level
                // statement; ensure we don't fold a following operator into it
                // by preventing the operator-led continuation for function defs.
                let is_function_def = matches!(expr, Expr::Function { .. });

                // If there's a semicolon and the token after it is an
                // operator we understand, consume the semicolon and fold
                // the operator + rhs into the current expression.
                loop {
                    // Need to check there's at least one token after the semicolon
                    if let Token::Semicolon = parser.peek() {
                        // safe index check
                        let next_idx = parser.pos + 1;
                        if let Some(next_tok) = parser.tokens.get(next_idx) {
                            if Parser::precedence(next_tok).is_some() && !is_function_def {
                                // consume the semicolon and operator
                                parser.next(); // semicolon
                                let op_tok = parser.next().clone();
                                let (prec, left_assoc) = Parser::precedence(&op_tok).unwrap();
                                let next_min = if left_assoc { prec + 1 } else { prec };
                                let right = parser.parse_prec(next_min)?;
                                let op = match op_tok {
                                    Token::Plus => BinaryOp::Add,
                                    Token::Minus => BinaryOp::Sub,
                                    Token::Star => BinaryOp::Mul,
                                    Token::Slash => BinaryOp::Div,
                                    Token::Gt => BinaryOp::Greater,
                                    Token::Lt => BinaryOp::Less,
                                    Token::EqComp => BinaryOp::Equal,
                                    _ => unreachable!(),
                                };
                                expr = Expr::Binary { left: Box::new(expr), op, right: Box::new(right) };
                                // after folding, continue in case of chained ops
                                continue;
                            }
                        }
                    }
                    break;
                }

                results.push(expr);
                // consume an optional semicolon after the expression if present
                if let Token::Semicolon = parser.peek() { parser.next(); }
            }
        }
    }

    for expr in results.iter(){
        println!("Parsed expression: {}", expr);
    }

    Ok(results)
    /*match p.peek() {
        Token::Eof => Ok(Vec<Expr>),
        t => Err(format!("Unexpected trailing token: {:?}", t)),
    }*/
}
