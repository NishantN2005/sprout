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
    Target::initialize_native(&InitializationConfig::default())
        .expect("Failed to initialize native target");
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

    let execution_engine = llvm_module
        .create_jit_execution_engine(OptimizationLevel::None)
        .map_err(|e| format!("Failed to create JIT engine: {:?}", e))?;

    unsafe {
        let addr = execution_engine
            .get_function_address("main")
            .map_err(|e| format!("Failed to get 'main' symbol: {:?}", e))?;

        let func: extern "C" fn() -> i64 = std::mem::transmute(addr);
        Ok(func())
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

    for inst in &ir_func.body {
        match inst {
            Inst::Const { dst, value } => {
                let v = i64_type.const_int(*value as u64, true);
                set_val(&mut values, *dst, v);
            }
            Inst::Boolean {dst, value} => {
                let v = if *value {
                    i64_type.const_int(1, false)
                }else{
                    i64_type.const_int(0, false)
                };
                set_val(&mut values, *dst, v);
            }
            Inst::Greater { dst, lhs, rhs } => {
                let l = get_val(&values, *lhs)?;
                let r = get_val(&values, *rhs)?;
                let cmp = builder
                    .build_int_compare(inkwell::IntPredicate::SGT, l, r, "gttmp")
                    .expect("build_int_compare failed");
                let v = builder
                    .build_int_z_extend(cmp, i64_type, "booltmp")
                    .expect("build_int_z_extend failed");
                set_val(&mut values, *dst, v);
            }
            Inst::Less { dst, lhs, rhs } => {
                let l = get_val(&values, *lhs)?;
                let r = get_val(&values, *rhs)?;
                let cmp = builder
                    .build_int_compare(inkwell::IntPredicate::SLT, l, r, "lttmp")
                    .expect("build_int_compare failed");
                let v = builder
                    .build_int_z_extend(cmp, i64_type, "booltmp")
                    .expect("build_int_z_extend failed");
                set_val(&mut values, *dst, v);
            }
            Inst::Equal { dst, lhs, rhs } => {
                let l = get_val(&values, *lhs)?;
                let r = get_val(&values, *rhs)?;
                let cmp = builder
                    .build_int_compare(inkwell::IntPredicate::EQ, l, r, "eqtmp")
                    .expect("build_int_compare failed");
                let v = builder
                    .build_int_z_extend(cmp, i64_type, "booltmp")
                    .expect("build_int_z_extend failed");
                set_val(&mut values, *dst, v);
            }
            Inst::Add { dst, lhs, rhs } => {
                let l = get_val(&values, *lhs)?;
                let r = get_val(&values, *rhs)?;
                let v = builder
                    .build_int_add(l, r, "addtmp")
                    .expect("build_int_add failed");
                set_val(&mut values, *dst, v);
            }

            Inst::Sub { dst, lhs, rhs } => {
                let l = get_val(&values, *lhs)?;
                let r = get_val(&values, *rhs)?;
                let v = builder
                    .build_int_sub(l, r, "subtmp")
                    .expect("build_int_sub failed");
                set_val(&mut values, *dst, v);
            }

            Inst::Div { dst, lhs, rhs } => {
                let l = get_val(&values, *lhs)?;
                let r = get_val(&values, *rhs)?;
                let v = builder
                    .build_int_signed_div(l, r, "divtmp")
                    .expect("build_int_signed_div failed");
                set_val(&mut values, *dst, v);
            }

            Inst::Mul { dst, lhs, rhs } => {
                let l = get_val(&values, *lhs)?;
                let r = get_val(&values, *rhs)?;
                let v = builder
                    .build_int_mul(l, r, "multmp")
                    .expect("build_int_mul failed");
                set_val(&mut values, *dst, v);
            }

            Inst::Store { name, src } => {
                let val = get_val(&values, *src)?;

                let ptr = vars.entry(name.clone()).or_insert_with(|| {
                    build_entry_alloca(context, builder, llvm_func, i64_type, name)
                });

                builder
                    .build_store(*ptr, val)
                    .expect("build_store failed");
            }

            Inst::Load { dst, name } => {
                let ptr = vars
                    .get(name)
                    .ok_or_else(|| format!("load of undefined variable '{name}'"))?;


                let loaded = builder
                    .build_load(i64_type, *ptr, &format!("load_{name}"))
                    .expect("build_load failed")
                    .into_int_value();

                set_val(&mut values, *dst, loaded);
            }

            Inst::Return { src } => {
                let v = get_val(&values, *src)?;
                builder.build_return(Some(&v));
                return Ok(()); // stop after first return
            }

            Inst::Call { .. } => {
                return Err("Call lowering not implemented yet".into());
            }
        }
    }

    // default: if no return seen, return 0
    let zero = i64_type.const_int(0, false);
    builder.build_return(Some(&zero));
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



