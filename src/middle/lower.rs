use crate::frontend::ast::{Expr, UnaryOp, BinaryOp};
use crate::middle::ir::{Module, Function, Inst, ValueId};

pub fn lower_program_to_module(exprs: &[Expr]) -> Module {
    let mut module = Module::new();
    let mut func = Function::new("main".to_string());

    let mut last: Option<ValueId> = None;

    for e in exprs {
        let v = lower_expr(e, &mut func);
        last = Some(v);
    }

    let result = match last {
        Some(v) => v,
        None => {
            let dst = func.fresh_value();
            func.body.push(Inst::Const { dst, value: 0 });
            dst
        }
    };

    func.body.push(Inst::Return { src: result });
    module.add_function(func);
    module
}

fn lower_expr(expr: &Expr, func: &mut Function) -> ValueId {
    match expr {
        Expr::If {cond, body, else_branch } => {
            // lower condition into the function's body
            let cond_val = lower_expr(cond, func);

            // prepare then_insts by lowering the body into a temporary Vec
            let mut then_insts: Vec<Inst> = Vec::new();
            let then_val = lower_into(body, func, &mut then_insts);

            // create a temp name based on the dst id that we'll use to store branch result
            let dst = func.fresh_value();
            let temp_name = format!("__if_tmp_{}", dst.get_usize());

            // append a store to temp at the end of then_insts
            then_insts.push(Inst::Store { name: temp_name.clone(), src: then_val });

            // else: if present lower into else_insts, otherwise default to 0
            let mut else_insts: Vec<Inst> = Vec::new();
            if let Some(else_expr) = else_branch {
                let else_val = lower_into(else_expr, func, &mut else_insts);
                else_insts.push(Inst::Store { name: temp_name.clone(), src: else_val });
            } else {
                let else_val = func.fresh_value();
                else_insts.push(Inst::Const { dst: else_val, value: 0 });
                else_insts.push(Inst::Store { name: temp_name.clone(), src: else_val });
            }

            // emit conditional instruction which will be expanded in codegen
            func.body.push(Inst::Conditional { cond: cond_val, body: then_insts, else_insts, dst });

            // The Conditional codegen will load the temp into dst; return dst here
            dst
        }
        Expr::Number(n) => {
            let dst = func.fresh_value();
            func.body.push(Inst::Const { dst, value: *n });
            dst
        }
        Expr::Ident(name) => {
            let dst = func.fresh_value();
            //boolean check
            if name == "true" {
                func.body.push(Inst::Boolean { dst, value: true });
            }else if name == "false" {
                func.body.push(Inst::Boolean { dst, value: false });
            }else{
                func.body.push(Inst::Load { dst, name: name.clone()});
            }
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
            let rhs = lower_expr(right, func);
            let dst = func.fresh_value();

            match op {
                BinaryOp::Assign => {
                    let name = match &**left{
                        Expr::Ident(n) => n.clone(),
                        _ => panic!("Left side of assignment must be an identifier!"),
                    };
                    func.body.push(Inst::Store { name: name.clone(), src: rhs });
                    func.body.push(Inst::Load { dst, name });

                },
                BinaryOp::Add => {
                    let lhs = lower_expr(left, func);
                    func.body.push(Inst::Add { dst, lhs, rhs })
                },
                BinaryOp::Sub => {
                    let lhs = lower_expr(left, func);
                    func.body.push(Inst::Sub { dst, lhs, rhs })
                },
                BinaryOp::Mul =>{
                    let lhs = lower_expr(left, func);
                    func.body.push(Inst::Mul { dst, lhs, rhs })
                },
                BinaryOp::Div => {
                    let lhs = lower_expr(left, func);
                    func.body.push(Inst::Div { dst, lhs, rhs })
                },
                BinaryOp::Greater => {
                    let lhs = lower_expr(left, func);
                    func.body.push(Inst::Greater {dst, lhs, rhs})
                },
                BinaryOp::Less => {
                    let lhs = lower_expr(left, func);
                    func.body.push(Inst::Less {dst, lhs, rhs})
                },
                BinaryOp::Equal => {
                    let lhs = lower_expr(left, func);
                    func.body.push(Inst::Equal {dst, lhs, rhs})
                }
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

// Lower an expression into a provided instruction vector. This uses the same
// Function `fresh_value` allocator so ValueIds remain unique across the function.
fn lower_into(expr: &Expr, func: &mut Function, out: &mut Vec<Inst>) -> ValueId {
    match expr {
        Expr::If { cond, body, else_branch } => {
            // lower condition into the provided out vector
            let cond_val = lower_into(cond, func, out);

            // then branch lowered into its own instruction vector
            let mut then_insts: Vec<Inst> = Vec::new();
            let then_val = lower_into(body, func, &mut then_insts);

            // allocate a dst and temp name that both branches will store into
            let dst = func.fresh_value();
            let temp_name = format!("__if_tmp_{}", dst.get_usize());
            then_insts.push(Inst::Store { name: temp_name.clone(), src: then_val });

            // else branch: if present, lower into else_insts; otherwise default to 0
            let mut else_insts: Vec<Inst> = Vec::new();
            if let Some(else_expr) = else_branch {
                let else_val = lower_into(else_expr, func, &mut else_insts);
                else_insts.push(Inst::Store { name: temp_name.clone(), src: else_val });
            } else {
                let else_val = func.fresh_value();
                else_insts.push(Inst::Const { dst: else_val, value: 0 });
                else_insts.push(Inst::Store { name: temp_name.clone(), src: else_val });
            }

            // push a Conditional inst into the provided vector
            out.push(Inst::Conditional { cond: cond_val, body: then_insts, else_insts, dst });
            dst
        }
        Expr::Number(n) => {
            let dst = func.fresh_value();
            out.push(Inst::Const { dst, value: *n });
            dst
        }
        Expr::Ident(name) => {
            let dst = func.fresh_value();
            if name == "true" {
                out.push(Inst::Boolean { dst, value: true });
            } else if name == "false" {
                out.push(Inst::Boolean { dst, value: false });
            } else {
                out.push(Inst::Load { dst, name: name.clone() });
            }
            dst
        }
        Expr::Unary { op, expr } => {
            let val = lower_into(expr, func, out);
            match op {
                UnaryOp::Neg => {
                    let zero = func.fresh_value();
                    out.push(Inst::Const { dst: zero, value: 0 });
                    let dst = func.fresh_value();
                    out.push(Inst::Sub { dst, lhs: zero, rhs: val });
                    dst
                }
            }
        }
        Expr::Binary { left, op, right } => {
            let rhs = lower_into(right, func, out);
            let dst = func.fresh_value();
            match op {
                BinaryOp::Assign => {
                    let name = match &**left {
                        Expr::Ident(n) => n.clone(),
                        _ => panic!("Left side of assignment must be an identifier!"),
                    };
                    out.push(Inst::Store { name: name.clone(), src: rhs });
                    out.push(Inst::Load { dst, name });
                }
                BinaryOp::Add => {
                    let lhs = lower_into(left, func, out);
                    out.push(Inst::Add { dst, lhs, rhs })
                }
                BinaryOp::Sub => {
                    let lhs = lower_into(left, func, out);
                    out.push(Inst::Sub { dst, lhs, rhs })
                }
                BinaryOp::Mul => {
                    let lhs = lower_into(left, func, out);
                    out.push(Inst::Mul { dst, lhs, rhs })
                }
                BinaryOp::Div => {
                    let lhs = lower_into(left, func, out);
                    out.push(Inst::Div { dst, lhs, rhs })
                }
                BinaryOp::Greater => {
                    let lhs = lower_into(left, func, out);
                    out.push(Inst::Greater { dst, lhs, rhs })
                }
                BinaryOp::Less => {
                    let lhs = lower_into(left, func, out);
                    out.push(Inst::Less { dst, lhs, rhs })
                }
                BinaryOp::Equal => {
                    let lhs = lower_into(left, func, out);
                    out.push(Inst::Equal { dst, lhs, rhs })
                }
            }
            dst
        }
        Expr::Call { callee, args } => {
            let callee_name = match &**callee {
                Expr::Ident(name) => name.clone(),
                _ => panic!("non-ident callee not supported yet!"),
            };
            let arg_ids: Vec<ValueId> = args.iter().map(|a| lower_into(a, func, out)).collect();
            let dst = func.fresh_value();
            out.push(Inst::Call { dst, callee: callee_name, args: arg_ids });
            dst
        }
    }
}