use crate::middle::ir::{Module, Function};
use std::collections::HashMap;

pub fn optimize_module(module: &mut Module)-> &mut Module {

    for func in module.functions.iter_mut(){
        constant_folding(func);
    }
    module
}

pub fn constant_folding(&mut function: &mut Function){
    let mut const_map = Hashmap::new();
    let mut new_body = Vec::new();

    for expr in function.body.iter(){
        match expr{
            Inst::Const {dst, value} =>{
                const_map.insert(*dst, *value);
                new_body.push(expr.clone());
            }
            Inst::Add {dst, lhs, rhs} =>{
                if let (Some(lv), Some(rv)) = (const_map.get(lhs), const_map.get(rhs)){
                    let res = lv + rv;
                    const_map.insert(*dst, res);
                    new_body.push(Inst::Const {dst: *dst, value: res});
                }
            }
            Inst::Sub {dst, lhs, rhs} =>{
                if let (Some(lv), Some(rv)) = (const_map.get(lhs), const_map.get(rhs)){
                    let res = lv - rv;
                    const_map.insert(*dst, res);
                    new_body.push(Inst::Const {dst: *dst, value: res});
                }
            }
            Inst::Mul { dst, lhs, rhs } => {
                if let (Some(lv), Some(rv)) = (const_map.get(lhs), const_map.get(rhs)) {
                    let res = lv * rv;
                    const_map.insert(*dst, res);
                    new_body.push(Inst::Const { dst: *dst, value: res });
                }
            }

            Inst::Div { dst, lhs, rhs } => {
                if let (Some(lv), Some(rv)) = (const_map.get(lhs), const_map.get(rhs)) {
                    // avoid folding division by zero at compile time
                    if *rv != 0 {
                        let res = lv / rv;
                        const_map.insert(*dst, res);
                        new_body.push(Inst::Const { dst: *dst, value: res });
                    }else{
                        new_body.push(expr.clone());
                    }
                }
            }


        }
    }
    function.body = new_body; 
}