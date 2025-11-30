use std::collections::HashMap;
use inkwell::targets::{InitializationConfig, Target};


use inkwell::{
    builder::Builder,
    context::Context,
    module::Module as LlvmModule,
    types::IntType,
    values::{FunctionValue, IntValue, PointerValue},
    OptimizationLevel
};

use crate::middle::ir::{Module as IrModule, Function as IrFunction, Inst, ValueId};
pub fn init_llvm() {
    match Target::initialize_native(&InitializationConfig::default()) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Warning: failed to initialize native LLVM target: {e}");
            eprintln!("JIT execution may fail. If you need JIT, install a compatible LLVM and set LLVM_SYS_<ver>_PREFIX environment variable.");
        }
    }
}
pub fn jit_run_main(ir: &IrModule) -> Result<i64, String> {
    //find IR main
    let main_ir = ir
        .functions
        .iter()
        .find(|f| f.name == "main")
        .ok_or("No main function found".to_string())?;

    //setup LLVM
    let context = Context::create();
    let llvm_module = context.create_module("sprout_module");
    let builder = context.create_builder();
    let i64_type = context.i64_type();

    //declare LLVM main function
    let llvm_main = declare_main_func(&context, &llvm_module, i64_type);

    //codegen main body
    codegen_function(&context, &builder, i64_type, llvm_main, main_ir)?;

    // Print the LLVM IR for debugging
    println!("LLVM IR:\n{}", llvm_module.print_to_string().to_string());

    // Try JIT execution
    match llvm_module.create_jit_execution_engine(OptimizationLevel::None) {
        Ok(execution_engine) => {
            unsafe {
                match execution_engine.get_function_address("main") {
                    Ok(addr) => {
                        let func: extern "C" fn() -> i64 = std::mem::transmute(addr);
                        Ok(func())
                    }
                    Err(e) => Err(format!("Failed to get 'main' symbol: {:?}", e)),
                }
            }
        }
        Err(e) => Err(format!("Failed to create JIT engine: {:?}", e)),
    }
}


//helpers
fn declare_main_func<'ctx>(
    _context: &'ctx Context,
    module: &LlvmModule<'ctx>,
    i64_type: IntType<'ctx>,
) -> FunctionValue<'ctx> {
    let fn_type = i64_type.fn_type(&[], false);
    module.add_function("main", fn_type, None)
}

fn get_val<'ctx>(
    values: &Vec<Option<IntValue<'ctx>>>,
    id: ValueId,
) -> Result<IntValue<'ctx>, String> {
    let idx = id.get_usize();
    println!("Getting value for ValueId v{}", idx);
    values
        .get(idx)
        .and_then(|v| *v)
        .ok_or_else(|| format!("ValueId v{} not found", idx))
}

fn set_val<'ctx>(
    values: &mut Vec<Option<IntValue<'ctx>>>,
    id: ValueId,
    v: IntValue<'ctx>,
) {
    let idx = id.get_usize();
    if values.len() <= idx {
        values.resize(idx + 1, None);
    }
    values[idx] = Some(v);
}


fn codegen_function<'ctx>(
    context: &'ctx Context,
    builder: &Builder<'ctx>,
    i64_type: IntType<'ctx>,
    llvm_func: FunctionValue<'ctx>,
    ir_func: &IrFunction,
) -> Result<(), String> {
    // entry
    let entry_bb = context.append_basic_block(llvm_func, "entry");
    builder.position_at_end(entry_bb);

    // map ValueId to LLVM Values
    let mut values: Vec<Option<IntValue<'ctx>>> = Vec::new();

    // map var names to allocation pointers
    let mut vars: HashMap<String, PointerValue<'ctx>> = HashMap::new();

    // track if we've seen a return instruction
    let mut has_return = false;

    // helper to codegen a single instruction (used recursively for nested blocks)
    fn codegen_inst<'ctx>(
        context: &'ctx Context,
        builder: &Builder<'ctx>,
        i64_type: IntType<'ctx>,
        llvm_func: FunctionValue<'ctx>,
        inst: &Inst,
        values: &mut Vec<Option<IntValue<'ctx>>>,
        vars: &mut HashMap<String, PointerValue<'ctx>>,
    ) -> Result<(), String> {
        match inst {
            Inst::Const { dst, value } => {
                let v = i64_type.const_int(*value as u64, true);
                set_val(values, *dst, v);
                Ok(())
            }
            Inst::Boolean { dst, value } => {
                let v = i64_type.const_int(if *value { 1 } else { 0 }, false);
                set_val(values, *dst, v);
                Ok(())
            }
            Inst::Less { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?;
                let r = get_val(values, *rhs)?;
                let cmp = builder
                    .build_int_compare(inkwell::IntPredicate::SLT, l, r, "cmplt")
                    .expect("build_int_compare failed");
                let v = builder
                    .build_int_z_extend(cmp, i64_type, "zext")
                    .expect("build_int_z_extend failed");
                set_val(values, *dst, v);
                Ok(())
            }
            Inst::Greater { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?;
                let r = get_val(values, *rhs)?;
                let cmp = builder
                    .build_int_compare(inkwell::IntPredicate::SGT, l, r, "cmpgt")
                    .expect("build_int_compare failed");
                let v = builder
                    .build_int_z_extend(cmp, i64_type, "zext")
                    .expect("build_int_z_extend failed");
                set_val(values, *dst, v);
                Ok(())
            }
            Inst::Equal { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?;
                let r = get_val(values, *rhs)?;
                let cmp = builder
                    .build_int_compare(inkwell::IntPredicate::EQ, l, r, "cmpeq")
                    .expect("build_int_compare failed");
                let v = builder
                    .build_int_z_extend(cmp, i64_type, "zext")
                    .expect("build_int_z_extend failed");
                set_val(values, *dst, v);
                Ok(())
            }
            Inst::Add { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?;
                let r = get_val(values, *rhs)?;
                let v = builder
                    .build_int_add(l, r, "addtmp")
                    .expect("build_int_add failed");
                set_val(values, *dst, v);
                Ok(())
            }
            Inst::Sub { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?;
                let r = get_val(values, *rhs)?;
                let v = builder
                    .build_int_sub(l, r, "subtmp")
                    .expect("build_int_sub failed");
                set_val(values, *dst, v);
                Ok(())
            }
            Inst::Div { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?;
                let r = get_val(values, *rhs)?;
                let v = builder
                    .build_int_signed_div(l, r, "divtmp")
                    .expect("build_int_signed_div failed");
                set_val(values, *dst, v);
                Ok(())
            }
            Inst::Mul { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?;
                let r = get_val(values, *rhs)?;
                let v = builder
                    .build_int_mul(l, r, "multmp")
                    .expect("build_int_mul failed");
                set_val(values, *dst, v);
                Ok(())
            }
            Inst::Store { name, src } => {
                let val = get_val(values, *src)?;
                let ptr = vars.entry(name.clone()).or_insert_with(|| {
                    build_entry_alloca(context, builder, llvm_func, i64_type, name)
                });
                builder
                    .build_store(*ptr, val)
                    .expect("build_store failed");
                Ok(())
            }
            Inst::Load { dst, name } => {
                let ptr = vars
                    .get(name)
                    .ok_or_else(|| format!("load of undefined variable '{name}'"))?;
                let loaded = builder
                    .build_load(i64_type, *ptr, &format!("load_{name}"))
                    .expect("build_load failed")
                    .into_int_value();
                set_val(values, *dst, loaded);
                Ok(())
            }
            Inst::Return { src } => {
                let v = get_val(values, *src)?;
                let _ = builder.build_return(Some(&v));
                // indicate stop by returning early to caller
                Ok(())
            }
            Inst::Call { .. } => {
                Err("Call lowering not implemented yet".into())
            }
            Inst::Conditional { cond, body, else_insts, dst } => {
                // save current builder position (in case we're nested)
                let current_block = builder.get_insert_block();

                // compute condition value
                let cond_val = get_val(values, *cond)?;
                let zero = i64_type.const_int(0, false);
                let cond_bool = builder
                    .build_int_compare(inkwell::IntPredicate::NE, cond_val, zero, "ifcond")
                    .expect("build_int_compare failed");

                // create blocks
                let then_bb = context.append_basic_block(llvm_func, "if.then");
                let else_bb = context.append_basic_block(llvm_func, "if.else");
                let merge_bb = context.append_basic_block(llvm_func, "if.merge");

                // pre-create entry alloca for temp so both branches store to same slot
                let temp_name = format!("__if_tmp_{}", dst.get_usize());
                let temp_ptr = build_entry_alloca(context, builder, llvm_func, i64_type, &temp_name);
                // insert into vars if not already present
                vars.entry(temp_name.clone()).or_insert(temp_ptr);

                // branch from current position
                builder
                    .build_conditional_branch(cond_bool, then_bb, else_bb);

                // THEN
                builder.position_at_end(then_bb);
                let mut then_terminated = false;
                for i in body.iter() {
                    codegen_inst(context, builder, i64_type, llvm_func, i, values, vars)?;
                    // check if this instruction is a Return (terminates the block)
                    if matches!(i, Inst::Return { .. }) {
                        then_terminated = true;
                        break;
                    }
                }
                if !then_terminated {
                    let _ = builder.build_unconditional_branch(merge_bb);
                }

                // ELSE
                builder.position_at_end(else_bb);
                let mut else_terminated = false;
                for i in else_insts.iter() {
                    codegen_inst(context, builder, i64_type, llvm_func, i, values, vars)?;
                    // check if this instruction terminates the block
                    if matches!(i, Inst::Return { .. }) {
                        else_terminated = true;
                        break;
                    }
                    // if it's a Conditional, the builder is now at its merge block
                    // continue adding instructions there
                }
                if !else_terminated {
                    // branch from current position (may be else_bb or a nested conditional's merge)
                    let _ = builder.build_unconditional_branch(merge_bb);
                }

                // MERGE: load temp into dst
                builder.position_at_end(merge_bb);
                let ptr = vars.get(&temp_name).expect("temp ptr missing");
                let loaded = builder
                    .build_load(i64_type, *ptr, &format!("load_if_{}", dst.get_usize()))
                    .expect("build_load failed")
                    .into_int_value();
                set_val(values, *dst, loaded);
                
                // if we were called from within a parent block (nested conditional),
                // restore builder position to the merge block so parent can continue
                // (merge is now the "current" block for any code after this conditional)
                Ok(())
            }
        }
    }

    // iterate top-level body and codegen each instruction via helper
    for inst in &ir_func.body {
        codegen_inst(context, builder, i64_type, llvm_func, inst, &mut values, &mut vars)?;
    }

    // only emit default return if the current block doesn't already have a terminator
    // (a terminator is a return or branch instruction that ends a basic block)
    if let Some(current_block) = builder.get_insert_block() {
        if current_block.get_terminator().is_none() {
            // no terminator, so emit default return 0
            let zero = i64_type.const_int(0, false);
            let _ = builder.build_return(Some(&zero));
        }
    }
    Ok(())
}


fn build_entry_alloca<'ctx>(
    _context: &'ctx Context,
    builder: &Builder<'ctx>,
    func: FunctionValue<'ctx>,
    i64_type: IntType<'ctx>,
    name: &str,
) -> PointerValue<'ctx> {
    let entry = func.get_first_basic_block().unwrap();
    // save current insertion point
    let current_block = builder.get_insert_block().unwrap();

    // temporarily move builder to the beginning of the entry block
    if let Some(first_instr) = entry.get_first_instruction() {
        builder.position_before(&first_instr);
    } else {
        builder.position_at_end(entry);
    }

    let alloca = builder.build_alloca(i64_type, name).expect("Alloca Failed");

    // restore insertion point
    builder.position_at_end(current_block);

    alloca
}



