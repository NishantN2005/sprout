use std::collections::{HashMap, HashSet};

use inkwell::targets::{InitializationConfig, Target};

use inkwell::{
    builder::Builder,
    context::Context,
    module::Module as LlvmModule,
    types::IntType,
    values::{
        BasicMetadataValueEnum, BasicValue, BasicValueEnum, CallSiteValue, FunctionValue, IntValue,
        PointerValue,
    },
    AddressSpace, IntPredicate, OptimizationLevel,
};

use crate::middle::ir::{Function as IrFunction, Inst, Module as IrModule, ValueId};

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
    // find IR main
    let _main_ir = ir
        .functions
        .iter()
        .find(|f| f.name == "main")
        .ok_or("No main function found".to_string())?;

    // setup LLVM
    let context = Context::create();
    let llvm_module = context.create_module("sprout_module");
    let builder = context.create_builder();

    let i64_type = context.i64_type();
    let i1_type = context.bool_type();

    // Declare runtime helpers and external allocators
    let i8_ptr_type = context.ptr_type(AddressSpace::default());
    let malloc_type = i8_ptr_type.fn_type(&[i64_type.into()], false);
    llvm_module.add_function("malloc", malloc_type, None);

    let list_len_type = i64_type.fn_type(&[i8_ptr_type.into()], false);
    llvm_module.add_function("sprout_list_len", list_len_type, None);

    // Declare all IR functions first so calls can reference them
    let mut llvm_fns: HashMap<String, FunctionValue> = HashMap::new();
    for f in &ir.functions {
        let fn_type = i64_type.fn_type(&vec![i64_type.into(); f.params.len()], false);
        let fv = llvm_module.add_function(&f.name, fn_type, None);
        llvm_fns.insert(f.name.clone(), fv);
    }

    // codegen all functions
    for f in &ir.functions {
        let fv = *llvm_fns.get(&f.name).unwrap();
        codegen_function(
            &context,
            &builder,
            &llvm_module,
            i64_type,
            i1_type,
            fv,
            f,
            &llvm_fns,
        )?;
    }

    println!("LLVM IR:\n{}", llvm_module.print_to_string().to_string());

    match llvm_module.create_jit_execution_engine(OptimizationLevel::None) {
        Ok(execution_engine) => unsafe {
            match execution_engine.get_function_address("main") {
                Ok(addr) => {
                    let func: extern "C" fn() -> i64 = std::mem::transmute(addr);
                    Ok(func())
                }
                Err(e) => Err(format!("Failed to get 'main' symbol: {:?}", e)),
            }
        },
        Err(e) => {
            eprintln!(
                "Codegen/JIT error: Failed to create JIT engine: {:?}\nFalling back to IR interpreter.",
                e
            );
            interpret_main(ir)
        }
    }
}

// ------------------------- Interpreter fallback (kept) -------------------------
fn interpret_main(ir: &IrModule) -> Result<i64, String> {
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
        if values.len() <= idx {
            values.resize(idx + 1, None);
        }
        values[idx] = Some(v);
    }

    fn exec_block(
        body: &[Inst],
        values: &mut Vec<Option<RuntimeVal>>,
        vars: &mut HashMap<String, RuntimeVal>,
        module: &IrModule,
    ) -> Result<Option<i64>, String> {
        for inst in body {
            match inst {
                Inst::Const { dst, value } => set_val(values, *dst, RuntimeVal::Int(*value)),
                Inst::Boolean { dst, value } => {
                    set_val(values, *dst, RuntimeVal::Int(if *value { 1 } else { 0 }))
                }
                Inst::Add { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => {
                            set_val(values, *dst, RuntimeVal::Int(a + b))
                        }
                        _ => return Err("Add expects two ints".to_string()),
                    }
                }
                Inst::Sub { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => {
                            set_val(values, *dst, RuntimeVal::Int(a - b))
                        }
                        _ => return Err("Sub expects two ints".to_string()),
                    }
                }
                Inst::Mul { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => {
                            set_val(values, *dst, RuntimeVal::Int(a * b))
                        }
                        _ => return Err("Mul expects two ints".to_string()),
                    }
                }
                Inst::Div { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(_), RuntimeVal::Int(0)) => {
                            return Err("Division by zero".to_string())
                        }
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => {
                            set_val(values, *dst, RuntimeVal::Int(a / b))
                        }
                        _ => return Err("Div expects two ints".to_string()),
                    }
                }
                Inst::Greater { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => set_val(
                            values,
                            *dst,
                            RuntimeVal::Int(if a > b { 1 } else { 0 }),
                        ),
                        _ => return Err("Greater expects two ints".to_string()),
                    }
                }
                Inst::Less { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => set_val(
                            values,
                            *dst,
                            RuntimeVal::Int(if a < b { 1 } else { 0 }),
                        ),
                        _ => return Err("Less expects two ints".to_string()),
                    }
                }
                Inst::Equal { dst, lhs, rhs } => {
                    let l = get_val(values, *lhs)?;
                    let r = get_val(values, *rhs)?;
                    match (l, r) {
                        (RuntimeVal::Int(a), RuntimeVal::Int(b)) => set_val(
                            values,
                            *dst,
                            RuntimeVal::Int(if a == b { 1 } else { 0 }),
                        ),
                        _ => return Err("Equal expects two ints".to_string()),
                    }
                }

                Inst::Store { name, src } => {
                    let v = get_val(values, *src)?;
                    vars.insert(name.clone(), v);
                }
                Inst::Load { dst, name } => {
                    let v = vars
                        .get(name)
                        .ok_or_else(|| format!("load of undefined variable '{}'", name))?
                        .clone();
                    set_val(values, *dst, v);
                }
                Inst::Return { src } => {
                    let v = get_val(values, *src)?;
                    return match v {
                        RuntimeVal::Int(i) => Ok(Some(i)),
                        _ => Err("Return of non-int from function".to_string()),
                    };
                }
                Inst::Conditional {
                    cond,
                    body,
                    else_insts,
                    dst: _,
                } => {
                    let cond_v = get_val(values, *cond)?;
                    let cond_is_true = match cond_v {
                        RuntimeVal::Int(i) => i != 0,
                        _ => true,
                    };
                    if cond_is_true {
                        if let Some(ret) = exec_block(body, values, vars, module)? {
                            return Ok(Some(ret));
                        }
                    } else if let Some(ret) = exec_block(else_insts, values, vars, module)? {
                        return Ok(Some(ret));
                    }
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
                    if callee == "range" {
                        if args.len() != 1 && args.len() != 2 {
                            return Err("range() expects 1 or 2 arguments".to_string());
                        }
                        let n_val = get_val(values, args[0])?;
                        match n_val {
                            RuntimeVal::Int(n) => {
                                let mut list_items = Vec::new();
                                for i in 0..n {
                                    list_items.push(RuntimeVal::Int(i));
                                }
                                set_val(values, *dst, RuntimeVal::List(list_items));
                            }
                            _ => return Err("range() expects an int argument".to_string()),
                        }
                        continue;
                    }

                    if callee == "len" {
                        if args.len() != 1 {
                            return Err("len() expects 1 argument".to_string());
                        }
                        let list_val = get_val(values, args[0])?;
                        match list_val {
                            RuntimeVal::List(ref items) => {
                                set_val(values, *dst, RuntimeVal::Int(items.len() as i64));
                            }
                            _ => return Err("len() expects a list argument".to_string()),
                        }
                        continue;
                    }

                    let mut arg_vals: Vec<RuntimeVal> = Vec::new();
                    for a in args {
                        arg_vals.push(get_val(values, *a)?);
                    }

                    let callee_fn = module
                        .functions
                        .iter()
                        .find(|f| f.name == *callee)
                        .ok_or_else(|| format!("call to undefined function '{}'", callee))?;

                    let mut callee_values: Vec<Option<RuntimeVal>> = Vec::new();
                    let mut callee_vars: HashMap<String, RuntimeVal> = HashMap::new();

                    for (i, pname) in callee_fn.params.iter().enumerate() {
                        if i < arg_vals.len() {
                            callee_vars.insert(pname.clone(), arg_vals[i].clone());
                        } else {
                            callee_vars.insert(pname.clone(), RuntimeVal::Int(0));
                        }
                    }

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

// ------------------------- LLVM codegen -------------------------

#[derive(Copy, Clone, Debug)]
enum LlvmVal<'ctx> {
    I64(IntValue<'ctx>),
    I1(IntValue<'ctx>),
}

impl<'ctx> LlvmVal<'ctx> {
    fn as_i1(
        self,
        builder: &Builder<'ctx>,
        i64_type: IntType<'ctx>,
    ) -> Result<IntValue<'ctx>, String> {
        match self {
            LlvmVal::I1(b) => Ok(b),
            LlvmVal::I64(x) => {
                let zero = i64_type.const_int(0, false);
                builder
                    .build_int_compare(IntPredicate::NE, x, zero, "i64_to_i1")
                    .map_err(|e| format!("{:?}", e))
            }
        }
    }

    fn as_i64(self, builder: &Builder<'ctx>, i64_type: IntType<'ctx>) -> Result<IntValue<'ctx>, String> {
        match self {
            LlvmVal::I64(x) => Ok(x),
            LlvmVal::I1(b) => builder
                .build_int_z_extend(b, i64_type, "i1_to_i64")
                .map_err(|e| format!("{:?}", e)),
        }
    }
}

fn get_val<'ctx>(values: &Vec<Option<LlvmVal<'ctx>>>, id: ValueId) -> Result<LlvmVal<'ctx>, String> {
    let idx = id.get_usize();
    values
        .get(idx)
        .and_then(|v| *v)
        .ok_or_else(|| format!("ValueId v{} not found", idx))
}

fn set_val<'ctx>(values: &mut Vec<Option<LlvmVal<'ctx>>>, id: ValueId, v: LlvmVal<'ctx>) {
    let idx = id.get_usize();
    if values.len() <= idx {
        values.resize(idx + 1, None);
    }
    values[idx] = Some(v);
}

fn collect_store_vars(insts: &[Inst], vars: &mut HashSet<String>) {
    for inst in insts {
        match inst {
            Inst::Store { name, .. } => {
                vars.insert(name.clone());
            }
            Inst::Conditional { body, else_insts, .. } => {
                collect_store_vars(body, vars);
                collect_store_vars(else_insts, vars);
            }
            _ => {}
        }
    }
}

fn build_entry_alloca_i64<'ctx>(
    builder: &Builder<'ctx>,
    function: FunctionValue<'ctx>,
    i64_type: IntType<'ctx>,
    name: &str,
) -> Result<PointerValue<'ctx>, String> {
    let entry = function.get_first_basic_block().unwrap();
    match entry.get_first_instruction() {
        Some(first_instr) => builder.position_before(&first_instr),
        None => builder.position_at_end(entry),
    }

    builder
        .build_alloca(i64_type, name)
        .map_err(|e| format!("{:?}", e))
}

fn callsite_basic<'ctx>(
    cs: inkwell::values::CallSiteValue<'ctx>,
) -> Result<inkwell::values::BasicValueEnum<'ctx>, String> {
    // Generic handling without depending on the internal enum name.
    // `try_as_basic_value()` returns a `ValueKind` with helper `basic()`.
    if let Some(bv) = cs.try_as_basic_value().basic() {
        Ok(bv)
    } else {
        Err("call returned void".to_string())
    }
}

fn codegen_block<'ctx>(
    context: &'ctx Context,
    builder: &Builder<'ctx>,
    llvm_module: &LlvmModule<'ctx>,
    llvm_func: FunctionValue<'ctx>,
    all_functions: &HashMap<String, FunctionValue<'ctx>>,
    i64_type: IntType<'ctx>,
    i1_type: IntType<'ctx>,
    insts: &[Inst],
    values: &mut Vec<Option<LlvmVal<'ctx>>>,
    vars: &mut HashMap<String, PointerValue<'ctx>>,
) -> Result<Option<LlvmVal<'ctx>>, String> {
    let mut last: Option<LlvmVal<'ctx>> = None;

    for inst in insts {
        if let Some(bb) = builder.get_insert_block() {
            if bb.get_terminator().is_some() {
                break;
            }
        }

        match inst {
            Inst::Const { dst, value } => {
                let v = i64_type.const_int(*value as u64, true);
                let lv = LlvmVal::I64(v);
                set_val(values, *dst, lv);
                last = Some(lv);
            }
            Inst::Boolean { dst, value } => {
                let v = i1_type.const_int(if *value { 1 } else { 0 }, false);
                let lv = LlvmVal::I1(v);
                set_val(values, *dst, lv);
                last = Some(lv);
            }

            Inst::Add { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?.as_i64(builder, i64_type)?;
                let r = get_val(values, *rhs)?.as_i64(builder, i64_type)?;
                let v = builder.build_int_add(l, r, "addtmp").map_err(|e| format!("{:?}", e))?;
                let lv = LlvmVal::I64(v);
                set_val(values, *dst, lv);
                last = Some(lv);
            }
            Inst::Sub { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?.as_i64(builder, i64_type)?;
                let r = get_val(values, *rhs)?.as_i64(builder, i64_type)?;
                let v = builder.build_int_sub(l, r, "subtmp").map_err(|e| format!("{:?}", e))?;
                let lv = LlvmVal::I64(v);
                set_val(values, *dst, lv);
                last = Some(lv);
            }
            Inst::Mul { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?.as_i64(builder, i64_type)?;
                let r = get_val(values, *rhs)?.as_i64(builder, i64_type)?;
                let v = builder.build_int_mul(l, r, "multmp").map_err(|e| format!("{:?}", e))?;
                let lv = LlvmVal::I64(v);
                set_val(values, *dst, lv);
                last = Some(lv);
            }
            Inst::Div { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?.as_i64(builder, i64_type)?;
                let r = get_val(values, *rhs)?.as_i64(builder, i64_type)?;
                let v = builder
                    .build_int_signed_div(l, r, "divtmp")
                    .map_err(|e| format!("{:?}", e))?;
                let lv = LlvmVal::I64(v);
                set_val(values, *dst, lv);
                last = Some(lv);
            }

            Inst::Less { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?.as_i64(builder, i64_type)?;
                let r = get_val(values, *rhs)?.as_i64(builder, i64_type)?;
                let b = builder
                    .build_int_compare(IntPredicate::SLT, l, r, "cmplt")
                    .map_err(|e| format!("{:?}", e))?;
                let lv = LlvmVal::I1(b);
                set_val(values, *dst, lv);
                last = Some(lv);
            }
            Inst::Greater { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?.as_i64(builder, i64_type)?;
                let r = get_val(values, *rhs)?.as_i64(builder, i64_type)?;
                let b = builder
                    .build_int_compare(IntPredicate::SGT, l, r, "cmpgt")
                    .map_err(|e| format!("{:?}", e))?;
                let lv = LlvmVal::I1(b);
                set_val(values, *dst, lv);
                last = Some(lv);
            }
            Inst::Equal { dst, lhs, rhs } => {
                let l = get_val(values, *lhs)?.as_i64(builder, i64_type)?;
                let r = get_val(values, *rhs)?.as_i64(builder, i64_type)?;
                let b = builder
                    .build_int_compare(IntPredicate::EQ, l, r, "cmpeq")
                    .map_err(|e| format!("{:?}", e))?;
                let lv = LlvmVal::I1(b);
                set_val(values, *dst, lv);
                last = Some(lv);
            }

            Inst::Store { name, src } => {
                let v = get_val(values, *src)?.as_i64(builder, i64_type)?;
                let ptr = vars.get(name).ok_or_else(|| format!("undefined variable '{}'", name))?;
                builder.build_store(*ptr, v);
                last = None;
            }
            Inst::Load { dst, name } => {
                let ptr = vars.get(name).ok_or_else(|| format!("undefined variable '{}'", name))?;
                let loaded = builder
                    .build_load(i64_type, *ptr, &format!("load_{}", name))
                    .map_err(|e| format!("{:?}", e))?
                    .into_int_value();
                let lv = LlvmVal::I64(loaded);
                set_val(values, *dst, lv);
                last = Some(lv);
            }

            Inst::Return { src } => {
                let v = get_val(values, *src)?.as_i64(builder, i64_type)?;
                builder.build_return(Some(&v));
                last = None;
            }

            Inst::Conditional { cond, body, else_insts, dst } => {
                let cond_v = get_val(values, *cond)?;
                let cond_i1 = cond_v.as_i1(builder, i64_type)?;

                let then_bb = context.append_basic_block(llvm_func, "then");
                let else_bb = context.append_basic_block(llvm_func, "else");
                let merge_bb = context.append_basic_block(llvm_func, "ifcont");

                builder
                    .build_conditional_branch(cond_i1, then_bb, else_bb)
                    .map_err(|e| format!("{:?}", e))?;

                // THEN
                builder.position_at_end(then_bb);
                let then_last = codegen_block(
                    context, builder, llvm_module, llvm_func, all_functions, i64_type, i1_type, body, values, vars,
                )?;
                let then_end = builder.get_insert_block().unwrap();
                if then_end.get_terminator().is_none() {
                    builder.build_unconditional_branch(merge_bb).map_err(|e| format!("{:?}", e))?;
                }

                // ELSE
                builder.position_at_end(else_bb);
                let else_last = codegen_block(
                    context, builder, llvm_module, llvm_func, all_functions, i64_type, i1_type, else_insts, values, vars,
                )?;
                let else_end = builder.get_insert_block().unwrap();
                if else_end.get_terminator().is_none() {
                    builder.build_unconditional_branch(merge_bb).map_err(|e| format!("{:?}", e))?;
                }

                // MERGE
                builder.position_at_end(merge_bb);

                let then_v = then_last.unwrap_or(LlvmVal::I64(i64_type.const_int(0, false)));
                let else_v = else_last.unwrap_or(LlvmVal::I64(i64_type.const_int(0, false)));

                let result = match (then_v, else_v) {
                    (LlvmVal::I1(t), LlvmVal::I1(e)) => {
                        let phi = builder.build_phi(i1_type, "ifval_i1").map_err(|e| format!("{:?}", e))?;
                        phi.add_incoming(&[(&t, then_end), (&e, else_end)]);
                        LlvmVal::I1(phi.as_basic_value().into_int_value())
                    }
                    (t, e) => {
                        let t64 = t.as_i64(builder, i64_type)?;
                        let e64 = e.as_i64(builder, i64_type)?;
                        let phi = builder.build_phi(i64_type, "ifval_i64").map_err(|e| format!("{:?}", e))?;
                        phi.add_incoming(&[(&t64, then_end), (&e64, else_end)]);
                        LlvmVal::I64(phi.as_basic_value().into_int_value())
                    }
                };

                set_val(values, *dst, result);
                last = Some(result);
            }

            Inst::MakeList { dst, elems } => {
                // list layout: [len:i64][elem0:i64]...
                let malloc_fn = llvm_module.get_function("malloc").ok_or("malloc not declared")?;

                let total_bytes = i64_type.const_int(((1 + elems.len()) * 8) as u64, false);
                let cs = builder.build_call(malloc_fn, &[total_bytes.into()], "malloccall").map_err(|e| format!("{:?}", e))?;
                let ptr_i8 = callsite_basic(cs)?.into_pointer_value();

                // NOTE: in LLVM 15+ pointers are opaque; use Context::ptr_type for casts.
                let ptr_ty = context.ptr_type(AddressSpace::default());
                let ptr_i64 = builder
                    .build_bit_cast(ptr_i8, ptr_ty, "list_ptr")
                    .map_err(|e| format!("{:?}", e))?
                    .into_pointer_value();

                // store len at [0]
                let len_val = i64_type.const_int(elems.len() as u64, false);
                builder.build_store(ptr_i64, len_val);

                // treat as ptr to i64 elements for GEP math; still opaque ptr in type system
                for (i, e) in elems.iter().enumerate() {
                    let ev = get_val(values, *e)?.as_i64(builder, i64_type)?;
                    let idx = i64_type.const_int((i as u64) + 1, false);
                    let elem_ptr = unsafe {
                        builder
                            .build_in_bounds_gep(i64_type, ptr_i64, &[idx], &format!("elem_ptr_{}", i))
                            .map_err(|e| format!("{:?}", e))?
                    };
                    builder.build_store(elem_ptr, ev);
                }

                let ptr_as_int = builder
                    .build_ptr_to_int(ptr_i64, i64_type, "list_as_int")
                    .map_err(|e| format!("{:?}", e))?;
                let lv = LlvmVal::I64(ptr_as_int);
                set_val(values, *dst, lv);
                last = Some(lv);
            }

            Inst::Call { dst, callee, args } => {
                // recreate i8* type locally (fixes your i8_ptr_type scope error)
                let i8_ptr_type = context.ptr_type(AddressSpace::default());

                if callee == "range" {
                    if args.len() != 1 && args.len() != 2 {
                        return Err("range() expects 1 or 2 arguments".to_string());
                    }
                    let n_val = get_val(values, args[0])?.as_i64(builder, i64_type)?;
                    let malloc_fn = llvm_module.get_function("malloc").ok_or("malloc not declared")?;

                    let one = i64_type.const_int(1, false);
                    let eight = i64_type.const_int(8, false);
                    let size = builder.build_int_add(n_val, one, "size").map_err(|e| format!("{:?}", e))?;
                    let total_bytes = builder.build_int_mul(size, eight, "total_bytes").map_err(|e| format!("{:?}", e))?;

                    let cs = builder
                        .build_call(malloc_fn, &[total_bytes.into()], "range_malloc")
                        .map_err(|e| format!("{:?}", e))?;
                    let ptr_i8 = callsite_basic(cs)?.into_pointer_value();

                    let ptr_ty = context.ptr_type(AddressSpace::default());
                    let ptr = builder
                        .build_bit_cast(ptr_i8, ptr_ty, "range_ptr")
                        .map_err(|e| format!("{:?}", e))?
                        .into_pointer_value();

                    // store len
                    builder.build_store(ptr, n_val);

                    // loop counter alloca at entry
                    let i_alloca = build_entry_alloca_i64(builder, llvm_func, i64_type, "range_i")?;
                    builder.build_store(i_alloca, i64_type.const_int(0, false));

                    let loop_bb = context.append_basic_block(llvm_func, "range_loop");
                    let body_bb = context.append_basic_block(llvm_func, "range_loop_body");
                    let end_bb = context.append_basic_block(llvm_func, "range_loop_end");

                    builder.build_unconditional_branch(loop_bb).map_err(|e| format!("{:?}", e))?;
                    builder.position_at_end(loop_bb);

                    let i_val = builder
                        .build_load(i64_type, i_alloca, "i_load")
                        .map_err(|e| format!("{:?}", e))?
                        .into_int_value();

                    let cond_i1 = builder
                        .build_int_compare(IntPredicate::SLT, i_val, n_val, "i_lt_n")
                        .map_err(|e| format!("{:?}", e))?;
                    builder.build_conditional_branch(cond_i1, body_bb, end_bb).map_err(|e| format!("{:?}", e))?;

                    builder.position_at_end(body_bb);

                    let idx = builder.build_int_add(i_val, one, "idx").map_err(|e| format!("{:?}", e))?;
                    let elem_ptr = unsafe {
                        builder
                            .build_in_bounds_gep(i64_type, ptr, &[idx], "elem_ptr")
                            .map_err(|e| format!("{:?}", e))?
                    };
                    builder.build_store(elem_ptr, i_val);

                    let i_next = builder.build_int_add(i_val, one, "i_next").map_err(|e| format!("{:?}", e))?;
                    builder.build_store(i_alloca, i_next);
                    builder.build_unconditional_branch(loop_bb).map_err(|e| format!("{:?}", e))?;

                    builder.position_at_end(end_bb);

                    let ptr_as_int = builder
                        .build_ptr_to_int(ptr, i64_type, "range_result")
                        .map_err(|e| format!("{:?}", e))?;
                    let lv = LlvmVal::I64(ptr_as_int);
                    set_val(values, *dst, lv);
                    last = Some(lv);
                } else if callee == "len" {
                    if args.len() != 1 {
                        return Err("len() expects 1 argument".to_string());
                    }
                    let argv = get_val(values, args[0])?.as_i64(builder, i64_type)?;
                    let arg_ptr = builder
                        .build_int_to_ptr(argv, i8_ptr_type, "arg_ptr")
                        .map_err(|e| format!("{:?}", e))?;

                    let len_fn = llvm_module
                        .get_function("sprout_list_len")
                        .ok_or("sprout_list_len not declared")?;

                    let cs = builder
                        .build_call(len_fn, &[arg_ptr.into()], "call_len")
                        .map_err(|e| format!("{:?}", e))?;
                    let ret = callsite_basic(cs)?.into_int_value();

                    let lv = LlvmVal::I64(ret);
                    set_val(values, *dst, lv);
                    last = Some(lv);
                } else {
                    let func = *all_functions
                        .get(callee)
                        .ok_or_else(|| format!("undefined function '{}'", callee))?;

                    // inkwell 0.7 build_call wants &[BasicMetadataValueEnum]
                    let mut meta_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(args.len());
                    for a in args {
                        let v = get_val(values, *a)?.as_i64(builder, i64_type)?;
                        meta_args.push(v.into());
                    }

                    let cs = builder
                        .build_call(func, meta_args.as_slice(), "calltmp")
                        .map_err(|e| format!("{:?}", e))?;
                    let ret = callsite_basic(cs)?.into_int_value();

                    let lv = LlvmVal::I64(ret);
                    set_val(values, *dst, lv);
                    last = Some(lv);
                }
            }

            _ => return Err(format!("unimplemented instruction in codegen: {:?}", inst)),
        }
    }

    Ok(last)
}

fn codegen_function<'ctx>(
    context: &'ctx Context,
    builder: &Builder<'ctx>,
    llvm_module: &LlvmModule<'ctx>,
    i64_type: IntType<'ctx>,
    i1_type: IntType<'ctx>,
    llvm_func: FunctionValue<'ctx>,
    ir_func: &IrFunction,
    all_functions: &HashMap<String, FunctionValue<'ctx>>,
) -> Result<(), String> {
    let entry_bb = context.append_basic_block(llvm_func, "entry");
    builder.position_at_end(entry_bb);

    let mut values: Vec<Option<LlvmVal<'ctx>>> = Vec::new();
    let mut vars: HashMap<String, PointerValue<'ctx>> = HashMap::new();

    // Allocate variable slots (i64) at entry
    let mut var_names: HashSet<String> = HashSet::new();
    collect_store_vars(&ir_func.body, &mut var_names);
    for var_name in var_names {
        let ptr = build_entry_alloca_i64(builder, llvm_func, i64_type, &var_name)?;
        vars.insert(var_name, ptr);
    }

    // Bind parameters to i64 allocas
    for (i, pname) in ir_func.params.iter().enumerate() {
        let param_val = llvm_func
            .get_nth_param(i as u32)
            .expect("param missing")
            .into_int_value();

        let ptr = build_entry_alloca_i64(builder, llvm_func, i64_type, &format!("arg_{}", pname))?;
        builder.position_at_end(entry_bb);
        builder.build_store(ptr, param_val);
        vars.insert(pname.clone(), ptr);
    }

    builder.position_at_end(entry_bb);

    let _ = codegen_block(
        context,
        builder,
        llvm_module,
        llvm_func,
        all_functions,
        i64_type,
        i1_type,
        &ir_func.body,
        &mut values,
        &mut vars,
    )?;

    // If the current block has no terminator, return 0 by default.
    if let Some(bb) = builder.get_insert_block() {
        if bb.get_terminator().is_none() {
            builder.build_return(Some(&i64_type.const_int(0, false)));
        }
    }

    Ok(())
}
