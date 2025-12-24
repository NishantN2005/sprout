use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(i64),
    Ident(String),
    List(Vec<Expr>),
    Unary { op: UnaryOp, expr: Box<Expr> },
    Binary { left: Box<Expr>, op: BinaryOp, right: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
    Function { name: String, params: Vec<String>, body: Box<Expr> },
    If {cond: Box<Expr>, body: Box<Expr>, else_branch: Option<Box<Expr>>},
    For {var: String, iter: Box<Expr>, body: Box<Expr>},
    Break,
    Continue,
}

//add increment operation later
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp { Neg }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOp { Add, Sub, Mul, Div, Assign, Greater, Less, Equal }

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Number(n) => write!(f, "{}", n),
            Expr::Ident(s) => write!(f, "{}", s),
            Expr::List(elements) =>{
                write!(f, "[")?;
                for (i, e) in elements.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", e)?;
                }
                write!(f, "]")
            }
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
            Expr::If { cond, body, else_branch } => {
                if let Some(else_e) = else_branch {
                    write!(f, "if {} {} else {}", cond, body, else_e)
                } else {
                    write!(f, "if {} {}", cond, body)
                }
            }
            Expr::For { var, iter, body } => {
                write!(f, "for {} in {}: {}", var, iter, body)
            }
            Expr::Break => write!(f, "break"),
            Expr::Continue => write!(f, "continue"),
            Expr::Function { name, params, body } => {
                write!(f, "def {}(", name)?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", p)?;
                }
                write!(f, "): {}", body)
            }
        }
    }
}
