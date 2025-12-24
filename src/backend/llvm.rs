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

    // Declare runtime helpers and external allocators
    let i8_ptr_type = context.ptr_type(inkwell::AddressSpace::default());
    // malloc: i8* malloc(i64)
    let malloc_type = i8_ptr_type.fn_type(&[i64_type.into()], false);
    llvm_module.add_function("malloc", malloc_type, None);
    // sprout_list_len: i64 sprout_list_len(i8*)
    let list_len_type = i64_type.fn_type(&[i8_ptr_type.into()], false);
    llvm_module.add_function("sprout_list_len", list_len_type, None);

    // Declare all IR functions first so calls can reference them
    let mut llvm_fns: HashMap<String, FunctionValue> = HashMap::new();
    for f in &ir.functions {
        let fn_type = i64_type.fn_type(&vec![i64_type.into(); f.params.len()], false);
        let fv = llvm_module.add_function(&f.name, fn_type, None);
        llvm_fns.insert(f.name.clone(), fv);
    }

    let llvm_main = llvm_fns.get("main").cloned().ok_or("No main function found".to_string())?;

    //codegen all functions
    for f in &ir.functions {
        let fv = llvm_fns.get(&f.name).unwrap().clone();
        codegen_function(&context, &builder, i64_type, fv, f, &llvm_fns, &llvm_module)?;
    }

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
        Err(e) => {
            eprintln!("Codegen/JIT error: Failed to create JIT engine: {:?}\nFalling back to IR interpreter.", e);
            // Interpreter fallback: execute the IR directly to produce a result
            interpret_main(ir)
        }
    }
}

// Simple interpreter fallback for environments without a working JIT.
fn interpret_main(ir: &IrModule) -> Result<i64, String> {
    // find main
    let main = ir
        .functions
        .iter()
        .find(|f| f.name == "main")
        .ok_or("No main function found".to_string())?;

    #[derive(Clone, Debug)]
    enum RuntimeVal {
        Int(i64),
        List(Vec<RuntimeVal>),
    }

    let mut values: Vec<Option<RuntimeVal>> = Vec::new();
    let mut vars: HashMap<String, RuntimeVal> = HashMap::new();

    fn get_val(values: &Vec<Option<RuntimeVal>>, id: ValueId) -> Result<RuntimeVal, String> {
        values
            .get(id.get_usize())
            .and_then(|v| v.clone())
            .ok_or_else(|| format!("ValueId v{} not found", id.get_usize()))
    }

    fn set_val(values: &mut Vec<Option<RuntimeVal>>, id: ValueId, v: RuntimeVal) {
        let idx = id.get_usize();
        if values.len() <= idx { values.resize(idx + 1, None); }
        values[idx] = Some(v);
    }

    fn exec_block(body: &[Inst], values: &mut Vec<Option<RuntimeVal>>, vars: &mut HashMap<String, RuntimeVal>, module: &IrModule) -> Result<Option<i64>, String> {
        for inst in body {
            match inst {
                Inst::Const { dst, value } => set_val(values, *dst, RuntimeVal::Int(*value)),
                Inst::Boolean { dst, value } => set_val(values, *dst, RuntimeVal::Int(if *value { 1 } else { 0 })),
                Inst::Add { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => set_val(values, *dst, RuntimeVal::Int(a + b)),
                        _ => return Err("Add expects two ints".to_string()),
                    }
                }
                Inst::Sub { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => set_val(values, *dst, RuntimeVal::Int(a - b)),
                        _ => return Err("Sub expects two ints".to_string()),
                    }
                }
                Inst::Mul { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => set_val(values, *dst, RuntimeVal::Int(a * b)),
                        _ => return Err("Mul expects two ints".to_string()),
                    }
                }
                Inst::Div { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(_), RuntimeVal::Int(0)) => return Err("Division by zero".to_string()),
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => set_val(values, *dst, RuntimeVal::Int(a / b)),
                        _ => return Err("Div expects two ints".to_string()),
                    }
                }
                Inst::Greater { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => set_val(values, *dst, RuntimeVal::Int(if a > b { 1 } else { 0 })),
                        _ => return Err("Greater expects two ints".to_string()),
                    }
                }
                Inst::Less { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => set_val(values, *dst, RuntimeVal::Int(if a < b { 1 } else { 0 })),
                        _ => return Err("Less expects two ints".to_string()),
                    }
                }
                Inst::Equal { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => set_val(values, *dst, RuntimeVal::Int(if a == b { 1 } else { 0 })),
                        _ => return Err("Equal expects two ints".to_string()),
                    }
                }
                
                Inst::Store { name, src } => {
                    let v = get_val(values, *src)?;
                    vars.insert(name.clone(), v);
                }
                Inst::Load { dst, name } => {
                    let v = vars.get(name).ok_or_else(|| format!("load of undefined variable '{}'", name))?.clone();
                    set_val(values, *dst, v);
                }
                    Inst::Return { src } => {
                        let v = get_val(values, *src)?;
                        // only top-level return values are i64; for runtime values we expect Int
                        return match v {
                            RuntimeVal::Int(i) => Ok(Some(i)),
                            _ => Err("Return of non-int from function".to_string()),
                        };
                    }
                Inst::Conditional { cond, body, else_insts, dst } => {
                    let cond_v = get_val(values, *cond)?;
                    let cond_is_true = match cond_v { RuntimeVal::Int(i) => i != 0, _ => true };
                    if cond_is_true {
                        if let Some(ret) = exec_block(body, values, vars, module)? { return Ok(Some(ret)); }
                    } else {
                        if let Some(ret) = exec_block(else_insts, values, vars, module)? { return Ok(Some(ret)); }
                    }
                    // dst is expected to be set via a store/load pattern; do nothing here
                }
                Inst::MakeList { dst, elems } => {
                    let mut v: Vec<RuntimeVal> = Vec::new();
                    for e in elems {
                        let ev = get_val(values, *e)?;
                        v.push(ev);
                    }
                    set_val(values, *dst, RuntimeVal::List(v));
                }
                Inst::Call { dst, callee, args } => {
                    // evaluate args to RuntimeVal
                    let mut arg_vals: Vec<RuntimeVal> = Vec::new();
                    for a in args {
                        arg_vals.push(get_val(values, *a)?);
                    }

                    // find callee function
                    let callee_fn = module.functions.iter().find(|f| f.name == *callee)
                        .ok_or_else(|| format!("call to undefined function '{}'", callee))?;

                    // prepare new frame values and vars
                    let mut callee_values: Vec<Option<RuntimeVal>> = Vec::new();
                    let mut callee_vars: HashMap<String, RuntimeVal> = HashMap::new();

                    // bind params
                    for (i, pname) in callee_fn.params.iter().enumerate() {
                        if i < arg_vals.len() {
                            callee_vars.insert(pname.clone(), arg_vals[i].clone());
                        } else {
                            callee_vars.insert(pname.clone(), RuntimeVal::Int(0));
                        }
                    }

                    // execute callee
                    match exec_block(&callee_fn.body, &mut callee_values, &mut callee_vars, module)? {
                        Some(v) => set_val(values, *dst, RuntimeVal::Int(v)),
                        None => set_val(values, *dst, RuntimeVal::Int(0)),
                    }
                }
            }
        }
        Ok(None)
    }

    match exec_block(&main.body, &mut values, &mut vars, ir)? {
        Some(v) => Ok(v),
        None => Ok(0),
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
    //println!("Getting value for ValueId v{}", idx);
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
    all_functions: &HashMap<String, FunctionValue<'ctx>>,
    llvm_module: &LlvmModule<'ctx>,
) -> Result<(), String> {
    // entry
    let entry_bb = context.append_basic_block(llvm_func, "entry");
    builder.position_at_end(entry_bb);

    // map ValueId to LLVM Values
    let mut values: Vec<Option<IntValue<'ctx>>> = Vec::new();

    // map var names to allocation pointers
    let mut vars: HashMap<String, PointerValue<'ctx>> = HashMap::new();

    // bind incoming parameters into local allocas so loads/stores work
    for (i, pname) in ir_func.params.iter().enumerate() {
        let param_val = llvm_func.get_nth_param(i as u32).expect("param missing").into_int_value();
        let ptr = build_entry_alloca(context, builder, llvm_func, i64_type, &format!("arg_{}", pname));
        builder.position_at_end(entry_bb); // ensure store goes in entry
        builder.build_store(ptr, param_val);
        vars.insert(pname.clone(), ptr);
    }

    // track if we've seen a return instruction
    let _has_return = false;

    // helper to codegen a single instruction (used recursively for nested blocks)
    fn codegen_inst<'ctx>(
        context: &'ctx Context,
        builder: &Builder<'ctx>,
        i64_type: IntType<'ctx>,
        llvm_func: FunctionValue<'ctx>,
        inst: &Inst,
        values: &mut Vec<Option<IntValue<'ctx>>>,
        vars: &mut HashMap<String, PointerValue<'ctx>>,
        all_functions: &HashMap<String, FunctionValue<'ctx>>,
        module: &LlvmModule<'ctx>,
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
            Inst::MakeList { dst, elems } => {
                // Create a heap-allocated list layout:
                // [i64 len, i64 elem0, i64 elem1, ...]
                let malloc_fn = module.get_function("malloc").ok_or("malloc not declared")?;
                // total bytes = (1 + elems.len()) * 8
                let total_bytes = i64_type.const_int(((1 + elems.len()) * 8) as u64, false);
                let call_site = builder
                    .build_call(malloc_fn, &[total_bytes.into()], "malloccall")
                    .map_err(|e| format!("malloc call failed: {:?}", e))?;
                let value_kind = call_site.try_as_basic_value();
                let ptr = if let Some(bv) = value_kind.basic() {
                    if let inkwell::values::BasicValueEnum::PointerValue(pv) = bv {
                        pv
                    } else {
                        return Err("malloc returned non-pointer".to_string());
                    }
                } else {
                    return Err("malloc returned void".to_string());
                };

                // bitcast to i64* for stores
                let i64_ptr_type = context.ptr_type(inkwell::AddressSpace::default());
                let ptr_i64 = builder
                    .build_bit_cast(ptr, i64_ptr_type, "list_ptr_i64")
                    .map_err(|e| format!("build_bit_cast failed: {:?}", e))?
                    .into_pointer_value();

                // store length at index 0
                let len_val = i64_type.const_int(elems.len() as u64, false);
                let len_ptr = i64_ptr_type.const_zero();
                builder.build_store(ptr_i64, len_val);

                // store each element at index i+1
                for (i, e) in elems.iter().enumerate() {
                    let ev = get_val(values, *e)?;
                    let idx = i64_type.const_int((i as u64) + 1, false);
                    let elem_ptr_val = unsafe { builder.build_in_bounds_gep(i64_ptr_type, ptr_i64, &[idx], &format!("elem_ptr_{}", i)) }
                        .map_err(|e| format!("build_in_bounds_gep failed: {:?}", e))?;
                    builder.build_store(elem_ptr_val, ev);
                }

                // convert pointer to i64 so it can be stored in our IntValue slots
                let ptr_as_int = builder.build_ptr_to_int(ptr_i64, i64_type, "list_as_int")
                    .map_err(|e| format!("build_ptr_to_int failed: {:?}", e))?;
                set_val(values, *dst, ptr_as_int);
                Ok(())
            }
            Inst::Call { dst, callee, args } => {
                // builtin: len(list) -> call sprout_list_len
                if callee == "len" {
                    if args.len() != 1 {
                        return Err("len() expects 1 argument".to_string());
                    }
                    let i8_ptr_type = context.ptr_type(inkwell::AddressSpace::default());
                    // arg is stored as i64 containing pointer
                    let argv = get_val(values, args[0])?;
                    // int -> ptr
                    let arg_ptr = builder.build_int_to_ptr(argv, i8_ptr_type, "arg_ptr")
                        .map_err(|e| format!("build_int_to_ptr failed: {:?}", e))?;
                    let len_fn = module.get_function("sprout_list_len").ok_or("sprout_list_len not declared")?;
                    let call_site = builder
                        .build_call(len_fn, &[arg_ptr.into()], "call_len")
                        .map_err(|e| format!("call_len failed: {:?}", e))?;
                    let value_kind = call_site.try_as_basic_value();
                    if let Some(bv) = value_kind.basic() {
                        if let inkwell::values::BasicValueEnum::IntValue(iv) = bv {
                            set_val(values, *dst, iv);
                        } else {
                            return Err("sprout_list_len returned non-int".to_string());
                        }
                    } else {
                        return Err("sprout_list_len returned void".to_string());
                    }
                    return Ok(());
                }

                // normal function call
                let callee_fn = all_functions.get(callee).ok_or_else(|| format!("call to undefined function '{}'", callee))?;
                // prepare argument list
                let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum> = Vec::new();
                for a in args {
                    let v = get_val(values, *a)?;
                    llvm_args.push(v.into());
                }
                let call_site = builder.build_call(*callee_fn, &llvm_args, "calltmp")
                    .map_err(|e| format!("build_call failed: {:?}", e))?;
                let value_kind = call_site.try_as_basic_value();
                if let Some(bv) = value_kind.basic() {
                    if let inkwell::values::BasicValueEnum::IntValue(iv) = bv {
                        set_val(values, *dst, iv);
                    } else {
                        return Err("call returned non-int value".to_string());
                    }
                } else {
                    return Err("call returned void".to_string());
                }
                Ok(())
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
                    codegen_inst(context, builder, i64_type, llvm_func, i, values, vars, all_functions, module)?;
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
                    codegen_inst(context, builder, i64_type, llvm_func, i, values, vars, all_functions, module)?;
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
        codegen_inst(context, builder, i64_type, llvm_func, inst, &mut values, &mut vars, all_functions, llvm_module)?;
    }

    // Emit default return in a dedicated exit block so we never place a `ret`
    // directly inside a branch/merge block. If the current insertion block
    // lacks a terminator, branch it to `exit` and put the return there.
    if let Some(current_block) = builder.get_insert_block() {
        if current_block.get_terminator().is_none() {
            // create a dedicated exit block and branch current block to it
            let exit_bb = context.append_basic_block(llvm_func, "exit");
            let _ = builder.build_unconditional_branch(exit_bb);

            // emit the return from the exit block
            builder.position_at_end(exit_bb);
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



