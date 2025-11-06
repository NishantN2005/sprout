use crate::frontend::ast::{Expr, UnaryOp, BinaryOp};
use crate::middle::ir::{Module, Function, Inst, ValueId};

pub fn lower_expr_to_module(expr: &Expr) -> Module {
    let mut module = Module::new();
    let mut func = Function::new("main");

    let result = lower_expr(expr, &mut func);

    func.body.push(Inst::Ret {src: result});

    module.add_function(func);
    module
}

fn lower_expr(expr: &Expr, func: &mut Function) -> ValueId {
    match expr {
        Expr::Number(n) => {
            let dst = func.fresh_value();
            func.body.push(Inst::Const { dst, value: *n });
            dst
        }
        Expr::Ident(name) => {
            let dst = func.fresh_value();
            func.body.push(Inst::Load { name: name.clone(), src: dst });
            dst
        }
        Expr::Unary { op, expr } => {
            let val = lower_expr(expr, func);
            match op {
                //will add increment operation here later
                UnaryOp::Neg => {
                    let zero = func.fresh_value();
                    func.body.push(Inst::Const { dst: zero, value: 0 });
                    let dst = func.fresh_value();
                    func.body.push(Inst::Sub { dst, lhs: zero, rhs: val });
                    dst
                }
            }
        }
        Expr::Binary { left, op, right } => {
            let lhs = lower_expr(left, func);
            let rhs = lower_expr(right, func);
            let dst = func.fresh_value();
            match op {
                BinaryOp::Add => func.body.push(Inst::Add { dst, lhs, rhs }),
                BinaryOp::Sub => func.body.push(Inst::Sub { dst, lhs, rhs }),
                BinaryOp::Mul => func.body.push(Inst::Mul { dst, lhs, rhs }),
                BinaryOp::Div => func.body.push(Inst::Div { dst, lhs, rhs }),
            }
            dst
        }
        Expr::Call { callee, args } => {
            let callee_name = match &**callee{
                Expr::Ident(name) => name.clone(),
                _ => panic!("non-ident callee not supported yet!"),
            };

            let arg_ids: Vec<ValueId> = args.iter().map(|a| lower_expr(a, func)).collect();

            let dst = func.fresh_value();
            func.body.push(Inst::Call {
                dst,
                callee: callee_name,
                args: arg_ids,
            });

            dst
        }
    }
}