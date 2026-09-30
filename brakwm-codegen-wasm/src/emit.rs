use brakwm_core::Result;
use brakwm_ir_lir::lir::{
    BlockId, LirFunction, LirOpcode, LirOperand, LirProgram, LirType, VirtReg,
};
use std::collections::HashMap;

use crate::encode::*;

// WASM value type bytes
const I32: u8 = 0x7F;
const I64: u8 = 0x7E;
const F32: u8 = 0x7D;
const F64: u8 = 0x7C;

// opcodes
const OP_UNREACHABLE: u8 = 0x00;
const OP_BLOCK: u8 = 0x02;
const OP_LOOP: u8 = 0x03;
const OP_IF: u8 = 0x04;
const OP_ELSE: u8 = 0x05;
const OP_END: u8 = 0x0B;
const OP_BR: u8 = 0x0C;
const OP_BR_TABLE: u8 = 0x0E;
const OP_RETURN: u8 = 0x0F;
const OP_CALL: u8 = 0x10;
const OP_DROP: u8 = 0x1A;
const OP_LOCAL_GET: u8 = 0x20;
const OP_LOCAL_SET: u8 = 0x21;
const OP_GLOBAL_GET: u8 = 0x23;
const OP_GLOBAL_SET: u8 = 0x24;
const OP_I32_CONST: u8 = 0x41;
const OP_I64_CONST: u8 = 0x42;
const OP_F32_CONST: u8 = 0x43;
const OP_F64_CONST: u8 = 0x44;
const OP_I32_EQZ: u8 = 0x45;
const OP_I32_WRAP_I64: u8 = 0xA7;
const OP_I32_TRUNC_F32_S: u8 = 0xA8;
const OP_I32_TRUNC_F64_S: u8 = 0xAA;
const OP_I32_LOAD: u8 = 0x28;
const OP_I64_LOAD: u8 = 0x29;
const OP_F32_LOAD: u8 = 0x2A;
const OP_F64_LOAD: u8 = 0x2B;
const OP_I32_STORE: u8 = 0x36;
const OP_I64_STORE: u8 = 0x37;
const OP_F32_STORE: u8 = 0x38;
const OP_F64_STORE: u8 = 0x39;

const MIN_PAGES: u32 = 2;
const DATA_BASE: u32 = 0x400;
const STACK_BASE: u32 = 0x4000;

struct FuncType {
    params: Vec<u8>,
    results: Vec<u8>,
}

fn vt(t: &LirType) -> u8 {
    match t {
        LirType::I64 => I64,
        LirType::F32 => F32,
        LirType::F64 => F64,
        _ => I32,
    }
}

fn layout_strings(program: &LirProgram) -> Vec<u32> {
    let mut addr = DATA_BASE;
    program
        .string_table
        .iter()
        .map(|s| {
            let a = addr;
            addr += s.len() as u32 + 1;
            a
        })
        .collect()
}

fn extern_namespace(name: &str) -> (&'static str, String) {
    if name == "proc_exit" {
        ("wasi_snapshot_preview1", "proc_exit".to_string())
    } else {
        ("env", name.to_string())
    }
}

fn type_size(t: &LirType) -> u32 {
    match t {
        LirType::I64 | LirType::F64 => 8,
        _ => 4,
    }
}

fn struct_fields(program: &LirProgram, name: &str) -> HashMap<String, u32> {
    let mut map = HashMap::new();
    let mut off = 0u32;
    if let Some(s) = program.structs.iter().find(|s| s.name == name) {
        for (fname, fty) in &s.fields {
            map.insert(fname.clone(), off);
            off += type_size(fty);
        }
    }
    map
}

fn struct_size(program: &LirProgram, name: &str) -> u32 {
    if let Some(s) = program.structs.iter().find(|s| s.name == name) {
        s.fields.iter().map(|(_, t)| type_size(t)).sum()
    } else {
        8
    }
}

fn field_ty(program: &LirProgram, name: &str, fname: &str) -> Option<LirType> {
    program
        .structs
        .iter()
        .find(|s| s.name == name)?
        .fields
        .iter()
        .find(|(n, _)| n == fname)
        .map(|(_, t)| t.clone())
}

fn align4(n: u32) -> u32 {
    (n + 3) & !3
}

/// Per-function lowering state: register -> wasm local index, plus the pc/sp
/// local indices and function-index map for calls.
struct FnCtx {
    local_of: HashMap<VirtReg, u32>,
    pc: u32,
    sp_save: u32,
    func_index: HashMap<String, u32>,
}

impl FnCtx {
    fn local(&self, r: VirtReg) -> u32 {
        *self.local_of.get(&r).unwrap_or(&0)
    }
}

pub fn emit_module_bytes(program: &LirProgram) -> Result<Vec<u8>> {
    // Collect function types (imports first, then internals), dedup.
    let mut types: Vec<FuncType> = Vec::new();
    let mut type_key: HashMap<String, u32> = HashMap::new();
    let add_type = |t: FuncType, types: &mut Vec<FuncType>, tk: &mut HashMap<String, u32>| -> u32 {
        let key = format!(
            "{}|{}",
            t.params.iter().map(|b| *b as char).collect::<String>(),
            t.results.iter().map(|b| *b as char).collect::<String>()
        );
        if let Some(i) = tk.get(&key) {
            *i
        } else {
            let i = types.len() as u32;
            types.push(t);
            tk.insert(key, i);
            i
        }
    };

    // Import type indices.
    let mut import_type_idx: Vec<u32> = Vec::new();
    for e in &program.extern_functions {
        let ft = FuncType {
            params: e.params.iter().map(vt).collect(),
            results: if matches!(e.ret_ty, LirType::Void) {
                vec![]
            } else {
                vec![vt(&e.ret_ty)]
            },
        };
        import_type_idx.push(add_type(ft, &mut types, &mut type_key));
    }
    // A real WASI module needs `_start`, which calls `main` and then hands the
    // result to `wasi_snapshot_preview1.proc_exit`. Synthesize that import when
    // the program does not declare it itself, so the runner can drive the module
    // through the standard entry point.
    let main_idx = program.functions.iter().position(|f| f.name == "main");
    let has_start = main_idx.is_some() && !program.functions.iter().any(|f| f.name == "_start");
    let declares_proc_exit = program
        .extern_functions
        .iter()
        .any(|e| e.name == "proc_exit");
    let synth_proc_exit = has_start && !declares_proc_exit;

    let n_program_externs = program.extern_functions.len() as u32;
    let n_imports = n_program_externs + if synth_proc_exit { 1 } else { 0 };
    let proc_exit_idx = if synth_proc_exit {
        n_program_externs
    } else {
        program
            .extern_functions
            .iter()
            .position(|e| e.name == "proc_exit")
            .map(|i| i as u32)
            .unwrap_or(0)
    };

    // Internal function type indices.
    let mut func_type_idx: Vec<u32> = Vec::new();
    for f in &program.functions {
        let ft = FuncType {
            params: f.params.iter().map(|r| vt(&f.reg_types[*r])).collect(),
            results: if matches!(f.ret_ty, LirType::Void) {
                vec![]
            } else {
                vec![vt(&f.ret_ty)]
            },
        };
        func_type_idx.push(add_type(ft, &mut types, &mut type_key));
    }

    // `_start` has type `() -> ()` and `proc_exit` has type `(i32) -> ()`.
    let proc_exit_type_idx = if synth_proc_exit {
        Some(add_type(
            FuncType {
                params: vec![I32],
                results: vec![],
            },
            &mut types,
            &mut type_key,
        ))
    } else {
        None
    };
    let start_type_idx = if has_start {
        add_type(
            FuncType {
                params: vec![],
                results: vec![],
            },
            &mut types,
            &mut type_key,
        )
    } else {
        0
    };

    // Function index map: imports 0..n_imports, internals after.
    let mut func_index: HashMap<String, u32> = HashMap::new();
    for (i, e) in program.extern_functions.iter().enumerate() {
        func_index.insert(e.name.clone(), i as u32);
    }
    for (i, f) in program.functions.iter().enumerate() {
        func_index.insert(f.name.clone(), n_imports + i as u32);
    }

    // ---- Assemble sections ----
    let mut out: Vec<u8> = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];

    // Type section (1)
    {
        let mut p = Vec::new();
        leb_u32(types.len() as u32, &mut p);
        for t in &types {
            p.push(0x60);
            leb_u32(t.params.len() as u32, &mut p);
            p.extend_from_slice(&t.params);
            leb_u32(t.results.len() as u32, &mut p);
            p.extend_from_slice(&t.results);
        }
        section(1, &p, &mut out);
    }

    // Import section (2)
    if n_imports > 0 {
        let mut p = Vec::new();
        leb_u32(n_imports, &mut p);
        for (i, e) in program.extern_functions.iter().enumerate() {
            let (ns, name) = extern_namespace(&e.name);
            write_name(ns, &mut p);
            write_name(&name, &mut p);
            p.push(0x00); // func import
            leb_u32(import_type_idx[i], &mut p);
        }
        if synth_proc_exit {
            write_name("wasi_snapshot_preview1", &mut p);
            write_name("proc_exit", &mut p);
            p.push(0x00); // func import
            leb_u32(proc_exit_type_idx.unwrap(), &mut p);
        }
        section(2, &p, &mut out);
    }

    // Function section (3)
    {
        let mut p = Vec::new();
        let count = func_type_idx.len() + if has_start { 1 } else { 0 };
        leb_u32(count as u32, &mut p);
        for t in &func_type_idx {
            leb_u32(*t, &mut p);
        }
        if has_start {
            leb_u32(start_type_idx, &mut p);
        }
        section(3, &p, &mut out);
    }

    // Memory section (5)
    {
        let mut p = Vec::new();
        leb_u32(1, &mut p);
        p.push(0x00); // limits, no max
        leb_u32(MIN_PAGES, &mut p);
        section(5, &p, &mut out);
    }

    // Global section (6): one mutable i32 stack pointer
    {
        let mut p = Vec::new();
        leb_u32(1, &mut p);
        p.push(I32);
        p.push(0x01); // mutable
        p.push(OP_I32_CONST);
        leb_i32(STACK_BASE as i32, &mut p);
        p.push(OP_END);
        section(6, &p, &mut out);
    }

    // Export section (7)
    {
        let mut p = Vec::new();
        let n_exports = 1 + program.functions.len() + if has_start { 1 } else { 0 };
        leb_u32(n_exports as u32, &mut p);
        write_name("memory", &mut p);
        p.push(0x02); // memory
        leb_u32(0, &mut p);
        for (i, f) in program.functions.iter().enumerate() {
            write_name(&f.name, &mut p);
            p.push(0x00); // func
            leb_u32(n_imports + i as u32, &mut p);
        }
        if has_start {
            write_name("_start", &mut p);
            p.push(0x00); // func
            leb_u32(n_imports + program.functions.len() as u32, &mut p);
        }
        section(7, &p, &mut out);
    }

    // Code section (10)
    {
        let mut p = Vec::new();
        let count = program.functions.len() + if has_start { 1 } else { 0 };
        leb_u32(count as u32, &mut p);
        for f in &program.functions {
            let body = emit_function(f, program, &func_index);
            leb_u32(body.len() as u32, &mut p);
            p.extend_from_slice(&body);
        }
        if has_start {
            let mi = main_idx.unwrap();
            let body = emit_start(&program.functions[mi], n_imports + mi as u32, proc_exit_idx);
            leb_u32(body.len() as u32, &mut p);
            p.extend_from_slice(&body);
        }
        section(10, &p, &mut out);
    }

    // Data section (11)
    if !program.string_table.is_empty() {
        let addrs = layout_strings(program);
        let mut p = Vec::new();
        leb_u32(program.string_table.len() as u32, &mut p);
        for (i, s) in program.string_table.iter().enumerate() {
            p.push(0x00); // active, memidx 0
            p.push(OP_I32_CONST);
            leb_i32(addrs[i] as i32, &mut p);
            p.push(OP_END);
            leb_u32(s.len() as u32, &mut p);
            p.extend_from_slice(s.as_bytes());
        }
        section(11, &p, &mut out);
    }

    Ok(out)
}

/// Emit the WASI `_start` entry point: call `main`, then `proc_exit` with its
/// result. `main` returning void yields exit code 0. Parameters to `main` are
/// supplied as zero constants. `main_wasm_idx` is the WASM function index of
/// `main`, already offset past the import section.
fn emit_start(main_fn: &LirFunction, main_wasm_idx: u32, proc_exit_idx: u32) -> Vec<u8> {
    let mut code: Vec<u8> = Vec::new();
    leb_u32(0, &mut code); // no locals

    for p in &main_fn.params {
        let t = vt(&main_fn.reg_types[*p]);
        match t {
            I64 => {
                code.push(OP_I64_CONST);
                leb_i32(0, &mut code);
            }
            F32 => {
                code.push(OP_F32_CONST);
                code.extend_from_slice(&0f32.to_le_bytes());
            }
            F64 => {
                code.push(OP_F64_CONST);
                code.extend_from_slice(&0f64.to_le_bytes());
            }
            _ => {
                code.push(OP_I32_CONST);
                leb_i32(0, &mut code);
            }
        }
    }
    code.push(OP_CALL);
    leb_u32(main_wasm_idx, &mut code);

    // proc_exit only accepts an i32, so narrow any wider main result.
    match &main_fn.ret_ty {
        LirType::Void => {
            code.push(OP_I32_CONST);
            leb_i32(0, &mut code);
        }
        LirType::I64 => {
            code.push(OP_I32_WRAP_I64);
        }
        LirType::F32 => {
            code.push(OP_I32_TRUNC_F32_S);
        }
        LirType::F64 => {
            code.push(OP_I32_TRUNC_F64_S);
        }
        _ => {}
    }

    code.push(OP_CALL);
    leb_u32(proc_exit_idx, &mut code);

    // proc_exit never returns.
    code.push(OP_UNREACHABLE);
    code.push(OP_END);
    code
}

fn emit_function(
    f: &LirFunction,
    program: &LirProgram,
    func_index: &HashMap<String, u32>,
) -> Vec<u8> {
    // Build local mapping.
    let mut local_of: HashMap<VirtReg, u32> = HashMap::new();
    let mut next = 0u32;
    for p in &f.params {
        local_of.insert(*p, next);
        next += 1;
    }
    let n_params = next;
    for r in 0..f.reg_types.len() {
        if local_of.contains_key(&r) {
            continue;
        }
        local_of.insert(r, next);
        next += 1;
    }
    let pc = next;
    let sp_save = next + 1;

    let ctx = FnCtx {
        local_of,
        pc,
        sp_save,
        func_index: func_index.clone(),
    };

    let n = f.blocks.len();
    let uses_stack = f.blocks.iter().any(|b| {
        b.insts
            .iter()
            .any(|i| matches!(i.opcode, LirOpcode::Alloca | LirOpcode::StructInit))
    });

    let mut code: Vec<u8> = Vec::new();

    // Local declarations: one entry per non-param local slot.
    let n_locals = (next - n_params) + 2; // extra regs + pc + sp_save
    let mut slot_types: Vec<u8> = vec![I32; n_locals as usize];
    for r in 0..f.reg_types.len() {
        if let Some(&li) = ctx.local_of.get(&r) {
            if li >= n_params {
                slot_types[(li - n_params) as usize] = vt(&f.reg_types[r]);
            }
        }
    }
    leb_u32(n_locals, &mut code);
    for st in &slot_types {
        code.push(1); // run length
        code.push(*st);
    }

    // Entry: pc = 0
    code.push(OP_I32_CONST);
    leb_i32(0, &mut code);
    code.push(OP_LOCAL_SET);
    leb_u32(pc, &mut code);
    if uses_stack {
        code.push(OP_GLOBAL_GET);
        leb_u32(0, &mut code);
        code.push(OP_LOCAL_SET);
        leb_u32(sp_save, &mut code);
    }

    // n == 1 needs no dispatch: the single block is the whole body.
    if n == 1 {
        code.extend_from_slice(&emit_block(f, program, 0, &ctx));
    } else {
        code.push(OP_LOOP);
        code.push(0x40); // void blocktype
                         // open n blocks
        for _ in 0..n {
            code.push(OP_BLOCK);
            code.push(0x40);
        }
        // local.get pc
        code.push(OP_LOCAL_GET);
        leb_u32(pc, &mut code);
        // br_table: n targets, target[i] = n-1-i, default = 0
        code.push(OP_BR_TABLE);
        leb_u32(n as u32, &mut code);
        for i in 0..n {
            leb_u32((n - 1 - i) as u32, &mut code);
        }
        leb_u32(0, &mut code); // default
                               // bodies interleaved with ends
        for i in (0..n).rev() {
            code.push(OP_END); // end b_i
            let body = emit_block(f, program, i, &ctx);
            code.extend_from_slice(&body);
            // br loop (depth = i)
            code.push(OP_BR);
            leb_u32(i as u32, &mut code);
        }
        code.push(OP_END); // end loop
    }

    // Make the post-loop path unreachable so the function-end validates for
    // any result type.
    code.push(OP_UNREACHABLE);
    code.push(OP_END); // function end

    code
}

fn emit_block(f: &LirFunction, program: &LirProgram, bi: usize, ctx: &FnCtx) -> Vec<u8> {
    use LirOpcode as Op;
    let block = &f.blocks[bi];
    let mut out: Vec<u8> = Vec::new();
    let mut pending_cmp: Option<(VirtReg, VirtReg)> = None;
    let uses_stack = f.blocks.iter().any(|b| {
        b.insts
            .iter()
            .any(|i| matches!(i.opcode, Op::Alloca | Op::StructInit))
    });

    let reg = |o: &LirOperand| -> VirtReg {
        match o {
            LirOperand::Reg(r) => *r,
            _ => 0,
        }
    };
    let label = |o: &LirOperand| -> String {
        match o {
            LirOperand::Label(s) => s.clone(),
            _ => String::new(),
        }
    };
    let field = |o: &LirOperand| -> String {
        match o {
            LirOperand::Field(s) => s.clone(),
            _ => String::new(),
        }
    };

    // Emit a load of a register onto the stack.
    let get = |out: &mut Vec<u8>, r: VirtReg| {
        out.push(OP_LOCAL_GET);
        leb_u32(ctx.local(r), out);
    };
    let get_slot = |out: &mut Vec<u8>, slot: u32| {
        out.push(OP_LOCAL_GET);
        leb_u32(slot, out);
    };
    // Emit a store from top of stack into a register.
    let set = |out: &mut Vec<u8>, r: VirtReg| {
        out.push(OP_LOCAL_SET);
        leb_u32(ctx.local(r), out);
    };

    for inst in &block.insts {
        match inst.opcode {
            Op::Comment | Op::Push | Op::Pop => {}
            Op::Mov => {
                let d = inst.dest.unwrap();
                match &inst.operands[0] {
                    LirOperand::ImmI64(v) => {
                        let t = vt(&f.reg_types[d]);
                        push_const(t, *v, &mut out);
                        set(&mut out, d);
                    }
                    LirOperand::ImmF64(v) => {
                        let t = vt(&f.reg_types[d]);
                        push_const_f(t, *v, &mut out);
                        set(&mut out, d);
                    }
                    LirOperand::Reg(s) => {
                        get(&mut out, *s);
                        set(&mut out, d);
                    }
                    LirOperand::StringRef(i) => {
                        let addr = layout_strings(program)
                            .get(*i)
                            .copied()
                            .unwrap_or(DATA_BASE);
                        out.push(OP_I32_CONST);
                        leb_i32(addr as i32, &mut out);
                        set(&mut out, d);
                    }
                    _ => {}
                }
            }
            Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Div
            | Op::Mod
            | Op::FAdd
            | Op::FSub
            | Op::FMul
            | Op::FDiv => {
                let d = inst.dest.unwrap();
                let a = reg(&inst.operands[0]);
                let b = reg(&inst.operands[1]);
                get(&mut out, a);
                get(&mut out, b);
                let t = vt(&f.reg_types[d]);
                let op = binop_op(inst.opcode, t);
                out.push(op);
                set(&mut out, d);
            }
            Op::Neg => {
                let d = inst.dest.unwrap();
                let a = reg(&inst.operands[0]);
                let t = vt(&f.reg_types[d]);
                match t {
                    F32 | F64 => {
                        get(&mut out, a);
                        out.push(neg_op(t));
                    }
                    _ => {
                        // i32.sub computes (second) - (top), so the zero must
                        // be pushed before the operand to get 0 - a. Pushing
                        // the operand first yielded a - 0, i.e. no negation.
                        out.push(OP_I32_CONST);
                        leb_i32(0, &mut out);
                        get(&mut out, a);
                        out.push(0x6B); // 0 - a
                    }
                }
                set(&mut out, d);
            }
            Op::Not => {
                let d = inst.dest.unwrap();
                let a = reg(&inst.operands[0]);
                get(&mut out, a);
                out.push(OP_I32_EQZ);
                set(&mut out, d);
            }
            Op::And | Op::Or | Op::Xor | Op::Shl | Op::Shr => {
                let d = inst.dest.unwrap();
                let a = reg(&inst.operands[0]);
                let b = reg(&inst.operands[1]);
                get(&mut out, a);
                get(&mut out, b);
                let t = vt(&f.reg_types[d]);
                let op = int_op(inst.opcode, t);
                out.push(op);
                set(&mut out, d);
            }
            Op::Cmp => {
                let a = reg(&inst.operands[0]);
                let b = reg(&inst.operands[1]);
                pending_cmp = Some((a, b));
            }
            Op::SetEq | Op::SetNe | Op::SetLt | Op::SetLe | Op::SetGt | Op::SetGe => {
                let d = inst.dest.unwrap();
                let (a, b) = pending_cmp.take().unwrap_or((0, 0));
                get(&mut out, a);
                get(&mut out, b);
                let at = vt(&f.reg_types[a]);
                let op = cmp_op(inst.opcode, at);
                out.push(op);
                set(&mut out, d);
            }
            Op::Load => {
                let d = inst.dest.unwrap();
                let a = reg(&inst.operands[0]);
                get(&mut out, a);
                let t = vt(&f.reg_types[d]);
                out.push(load_op(t));
                leb_u32(0, &mut out); // align
                leb_u32(0, &mut out); // offset
                set(&mut out, d);
            }
            Op::Store => {
                let addr = reg(&inst.operands[0]);
                let val = reg(&inst.operands[1]);
                get(&mut out, addr);
                get(&mut out, val);
                let t = vt(&f.reg_types[val]);
                out.push(store_op(t));
                leb_u32(0, &mut out);
                leb_u32(0, &mut out);
            }
            Op::Alloca => {
                let d = inst.dest.unwrap();
                let size = match &inst.operands.first() {
                    Some(LirOperand::ImmI64(v)) => align4(*v as u32),
                    _ => 16,
                };
                // dest = sp; sp = sp - size
                out.push(OP_GLOBAL_GET);
                leb_u32(0, &mut out);
                set(&mut out, d);
                out.push(OP_GLOBAL_GET);
                leb_u32(0, &mut out);
                out.push(OP_I32_CONST);
                leb_i32(size as i32, &mut out);
                out.push(0x6B); // i32.sub
                out.push(OP_GLOBAL_SET);
                leb_u32(0, &mut out);
            }
            Op::StructInit => {
                let d = inst.dest.unwrap();
                let name = label(&inst.operands[0]);
                let fields = struct_fields(program, &name);
                let size = align4(struct_size(program, &name));
                out.push(OP_GLOBAL_GET);
                leb_u32(0, &mut out);
                set(&mut out, d);
                out.push(OP_GLOBAL_GET);
                leb_u32(0, &mut out);
                out.push(OP_I32_CONST);
                leb_i32(size as i32, &mut out);
                out.push(0x6B);
                out.push(OP_GLOBAL_SET);
                leb_u32(0, &mut out);
                let mut i = 1;
                while i + 1 < inst.operands.len() {
                    let fname = field(&inst.operands[i]);
                    let val = reg(&inst.operands[i + 1]);
                    let off = fields.get(&fname).copied().unwrap_or(0);
                    let ft = field_ty(program, &name, &fname)
                        .unwrap_or_else(|| f.reg_types[val].clone());
                    // addr = d + off
                    get(&mut out, d);
                    out.push(OP_I32_CONST);
                    leb_i32(off as i32, &mut out);
                    out.push(0x6A); // i32.add
                    get(&mut out, val);
                    let t = vt(&ft);
                    out.push(store_op(t));
                    leb_u32(0, &mut out);
                    leb_u32(0, &mut out);
                    i += 2;
                }
            }
            Op::GetField => {
                let d = inst.dest.unwrap();
                let obj = reg(&inst.operands[0]);
                let fname = field(&inst.operands[1]);
                let name = label(&inst.operands[2]);
                let fields = struct_fields(program, &name);
                let off = fields.get(&fname).copied().unwrap_or(0);
                let ft = field_ty(program, &name, &fname).unwrap_or_else(|| f.reg_types[d].clone());
                get(&mut out, obj);
                out.push(OP_I32_CONST);
                leb_i32(off as i32, &mut out);
                out.push(0x6A);
                let t = vt(&ft);
                out.push(load_op(t));
                leb_u32(0, &mut out);
                leb_u32(0, &mut out);
                set(&mut out, d);
            }
            Op::SetField => {
                let obj = reg(&inst.operands[0]);
                let fname = field(&inst.operands[1]);
                let val = reg(&inst.operands[2]);
                let name = label(&inst.operands[3]);
                let fields = struct_fields(program, &name);
                let off = fields.get(&fname).copied().unwrap_or(0);
                let ft =
                    field_ty(program, &name, &fname).unwrap_or_else(|| f.reg_types[val].clone());
                get(&mut out, obj);
                out.push(OP_I32_CONST);
                leb_i32(off as i32, &mut out);
                out.push(0x6A);
                get(&mut out, val);
                let t = vt(&ft);
                out.push(store_op(t));
                leb_u32(0, &mut out);
                leb_u32(0, &mut out);
            }
            Op::Call => {
                let callee = label(&inst.operands[0]);
                let args: Vec<VirtReg> = inst
                    .operands
                    .iter()
                    .skip(1)
                    .filter_map(|o| match o {
                        LirOperand::Reg(r) => Some(*r),
                        _ => None,
                    })
                    .collect();
                for a in &args {
                    get(&mut out, *a);
                }
                out.push(OP_CALL);
                let idx = *ctx.func_index.get(&callee).unwrap_or(&0);
                leb_u32(idx, &mut out);
                if let Some(d) = inst.dest {
                    set(&mut out, d);
                } else {
                    // discard the (possibly void) result
                    if is_void_callee(program, &callee) {
                        // nothing on stack
                    } else {
                        out.push(OP_DROP);
                    }
                }
            }
            Op::Ret => {
                if uses_stack {
                    get_slot(&mut out, ctx.sp_save);
                    out.push(OP_GLOBAL_SET);
                    leb_u32(0, &mut out);
                }
                match inst.operands.first() {
                    Some(LirOperand::Reg(r)) => {
                        get(&mut out, *r);
                    }
                    _ => {}
                }
                out.push(OP_RETURN);
            }
            Op::Jmp => {
                let target = label(&inst.operands[0]);
                let idx = block_index_by_ident(f, &target);
                out.push(OP_I32_CONST);
                leb_i32(idx as i32, &mut out);
                out.push(OP_LOCAL_SET);
                leb_u32(ctx.pc, &mut out);
            }
            Op::Br => {
                let cond = reg(&inst.operands[0]);
                let then_t = label(&inst.operands[1]);
                let else_t = label(&inst.operands[2]);
                let then_idx = block_index_by_ident(f, &then_t);
                let else_idx = block_index_by_ident(f, &else_t);
                get(&mut out, cond);
                out.push(OP_IF);
                out.push(0x40);
                // then
                out.push(OP_I32_CONST);
                leb_i32(then_idx as i32, &mut out);
                out.push(OP_LOCAL_SET);
                leb_u32(ctx.pc, &mut out);
                out.push(OP_ELSE);
                out.push(OP_I32_CONST);
                leb_i32(else_idx as i32, &mut out);
                out.push(OP_LOCAL_SET);
                leb_u32(ctx.pc, &mut out);
                out.push(OP_END);
            }
        }
    }

    out
}

fn is_void_callee(program: &LirProgram, name: &str) -> bool {
    if let Some(f) = program.functions.iter().find(|f| f.name == name) {
        return matches!(f.ret_ty, LirType::Void);
    }
    if let Some(e) = program.extern_functions.iter().find(|e| e.name == name) {
        return matches!(e.ret_ty, LirType::Void);
    }
    false
}

fn block_index_by_ident(f: &LirFunction, label: &str) -> usize {
    let id: BlockId = label
        .strip_prefix("block_")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    f.blocks.iter().position(|b| b.id == id).unwrap_or(0)
}

fn push_const(t: u8, v: i64, out: &mut Vec<u8>) {
    match t {
        I32 => {
            out.push(OP_I32_CONST);
            leb_i32(v as i32, out);
        }
        I64 => {
            out.push(OP_I64_CONST);
            leb_i64(v, out);
        }
        F32 => {
            out.push(OP_F32_CONST);
            leb_f32(v as f32, out);
        }
        F64 => {
            out.push(OP_F64_CONST);
            leb_f64(v as f64, out);
        }
        _ => {
            out.push(OP_I32_CONST);
            leb_i32(v as i32, out);
        }
    }
}

fn push_const_f(t: u8, v: f64, out: &mut Vec<u8>) {
    match t {
        I32 => {
            out.push(OP_I32_CONST);
            leb_i32(v as i32, out);
        }
        I64 => {
            out.push(OP_I64_CONST);
            leb_i64(v as i64, out);
        }
        F32 => {
            out.push(OP_F32_CONST);
            leb_f32(v as f32, out);
        }
        F64 => {
            out.push(OP_F64_CONST);
            leb_f64(v, out);
        }
        _ => {
            out.push(OP_I32_CONST);
            leb_i32(v as i32, out);
        }
    }
}

fn binop_op(op: LirOpcode, t: u8) -> u8 {
    match (op, t) {
        (LirOpcode::Add, I64) => 0x7C,
        (LirOpcode::Add, F32) => 0x92,
        (LirOpcode::Add, F64) => 0xA0,
        (LirOpcode::Add, _) => 0x6A,
        (LirOpcode::Sub, I64) => 0x7D,
        (LirOpcode::Sub, F32) => 0x93,
        (LirOpcode::Sub, F64) => 0xA1,
        (LirOpcode::Sub, _) => 0x6B,
        (LirOpcode::Mul, I64) => 0x7E,
        (LirOpcode::Mul, F32) => 0x94,
        (LirOpcode::Mul, F64) => 0xA2,
        (LirOpcode::Mul, _) => 0x6C,
        (LirOpcode::Div, I64) => 0x7F,
        (LirOpcode::Div, F32) => 0x95,
        (LirOpcode::Div, F64) => 0xA3,
        (LirOpcode::Div, _) => 0x6D,
        (LirOpcode::Mod, I64) => 0x81,
        (LirOpcode::Mod, _) => 0x6F,
        (LirOpcode::FAdd, F32) => 0x92,
        (LirOpcode::FAdd, F64) => 0xA0,
        (LirOpcode::FAdd, _) => 0x6A,
        (LirOpcode::FSub, F32) => 0x93,
        (LirOpcode::FSub, F64) => 0xA1,
        (LirOpcode::FSub, _) => 0x6B,
        (LirOpcode::FMul, F32) => 0x94,
        (LirOpcode::FMul, F64) => 0xA2,
        (LirOpcode::FMul, _) => 0x6C,
        (LirOpcode::FDiv, F32) => 0x95,
        (LirOpcode::FDiv, F64) => 0xA3,
        (LirOpcode::FDiv, _) => 0x6D,
        _ => 0x6A,
    }
}

fn neg_op(t: u8) -> u8 {
    match t {
        F32 => 0x8C,
        F64 => 0x9A,
        _ => 0x6B,
    }
}

fn int_op(op: LirOpcode, t: u8) -> u8 {
    match (op, t) {
        (LirOpcode::And, I64) => 0x83,
        (LirOpcode::And, _) => 0x71,
        (LirOpcode::Or, I64) => 0x84,
        (LirOpcode::Or, _) => 0x72,
        (LirOpcode::Xor, I64) => 0x85,
        (LirOpcode::Xor, _) => 0x73,
        (LirOpcode::Shl, I64) => 0x86,
        (LirOpcode::Shl, _) => 0x74,
        (LirOpcode::Shr, I64) => 0x87,
        (LirOpcode::Shr, _) => 0x75,
        _ => 0x71,
    }
}

fn cmp_op(op: LirOpcode, t: u8) -> u8 {
    match (op, t) {
        (LirOpcode::SetEq, I64) => 0x51,
        (LirOpcode::SetEq, F32) => 0x5B,
        (LirOpcode::SetEq, F64) => 0x61,
        (LirOpcode::SetEq, _) => 0x46,
        (LirOpcode::SetNe, I64) => 0x52,
        (LirOpcode::SetNe, F32) => 0x5C,
        (LirOpcode::SetNe, F64) => 0x62,
        (LirOpcode::SetNe, _) => 0x47,
        (LirOpcode::SetLt, I64) => 0x53,
        (LirOpcode::SetLt, F32) => 0x5D,
        (LirOpcode::SetLt, F64) => 0x63,
        (LirOpcode::SetLt, _) => 0x48,
        (LirOpcode::SetLe, I64) => 0x57,
        (LirOpcode::SetLe, F32) => 0x5F,
        (LirOpcode::SetLe, F64) => 0x65,
        (LirOpcode::SetLe, _) => 0x4C,
        (LirOpcode::SetGt, I64) => 0x55,
        (LirOpcode::SetGt, F32) => 0x5E,
        (LirOpcode::SetGt, F64) => 0x64,
        (LirOpcode::SetGt, _) => 0x4A,
        (LirOpcode::SetGe, I64) => 0x59,
        (LirOpcode::SetGe, F32) => 0x60,
        (LirOpcode::SetGe, F64) => 0x66,
        (LirOpcode::SetGe, _) => 0x4E,
        _ => 0x46,
    }
}

fn load_op(t: u8) -> u8 {
    match t {
        I64 => OP_I64_LOAD,
        F32 => OP_F32_LOAD,
        F64 => OP_F64_LOAD,
        _ => OP_I32_LOAD,
    }
}

fn store_op(t: u8) -> u8 {
    match t {
        I64 => OP_I64_STORE,
        F32 => OP_F32_STORE,
        F64 => OP_F64_STORE,
        _ => OP_I32_STORE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brakwm_core::SourceMap;
    use brakwm_frontend::parser::Parser;
    use brakwm_ir_hir::lower::HirLower;
    use brakwm_ir_lir::lower::LirLower;
    use brakwm_ir_mir::lower::MirLower;

    fn compile(src: &str) -> Vec<u8> {
        emit_module_bytes(&build_lir(src)).unwrap()
    }

    fn build_lir(src: &str) -> LirProgram {
        let sm = SourceMap::new("t.brk", src);
        let mut p = Parser::new();
        let ast = p.parse_source(&sm).unwrap();
        let hir = HirLower::new().lower(ast).unwrap();
        let mut ml = MirLower::new();
        let mir = ml.lower(hir).unwrap();
        let mut ll = LirLower::new();
        ll.set_file_id(0);
        ll.lower(mir)
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    /// Return the body bytes of the first function in the code section.
    fn first_code_body(data: &[u8]) -> Option<Vec<u8>> {
        let mut i = 8usize;
        while i < data.len() {
            let id = data[i];
            let (size, no) = read_leb(&data[i + 1..]);
            let body = &data[i + 1 + no..];
            if id == 10 {
                let (_count, co) = read_leb(body);
                let o = co;
                let (bsize, so) = read_leb(&body[o..]);
                let start = o + so;
                return Some(body[start..start + bsize as usize].to_vec());
            }
            i = i + 1 + no + size as usize;
        }
        None
    }

    /// Scan the export section for a named export.
    /// Walk the section list and return true when the export section (id 7)
    /// contains `want`.
    fn has_export(data: &[u8], want: &str) -> bool {
        let mut i = 8usize;
        while i < data.len() {
            let id = data[i];
            let (size, n) = read_leb(&data[i + 1..]);
            let start = i + 1 + n;
            let end = start + size as usize;
            if end > data.len() {
                return false;
            }
            if id == 7 {
                let body = &data[start..end];
                let (count, mut o) = read_leb(body);
                for _ in 0..count {
                    let (nlen, no) = read_leb(&body[o..]);
                    let name_start = o + no;
                    let name_end = name_start + nlen as usize;
                    if name_end > body.len() {
                        return false;
                    }
                    let name = &body[name_start..name_end];
                    o = name_end + 1; // skip the export kind byte
                    let (_idx, io) = read_leb(&body[o..]);
                    o += io;
                    if name == want.as_bytes() {
                        return true;
                    }
                }
                return false;
            }
            i = end;
        }
        false
    }

    fn read_leb(b: &[u8]) -> (u32, usize) {
        let mut result = 0u32;
        let mut shift = 0;
        for (i, &byte) in b.iter().enumerate() {
            result |= ((byte & 0x7F) as u32) << shift;
            if byte & 0x80 == 0 {
                return (result, i + 1);
            }
            shift += 7;
        }
        (result, b.len())
    }

    fn has_wasi_proc_exit_import(data: &[u8]) -> bool {
        let mut i = 8usize;
        while i < data.len() {
            let id = data[i];
            let (size, n) = read_leb(&data[i + 1..]);
            let start = i + 1 + n;
            let end = start + size as usize;
            if end > data.len() {
                return false;
            }
            if id == 2 {
                let body = &data[start..end];
                let (count, mut o) = read_leb(body);
                for _ in 0..count {
                    let (mlen, mo) = read_leb(&body[o..]);
                    let mstart = o + mo;
                    let mend = mstart + mlen as usize;
                    if mend > body.len() {
                        return false;
                    }
                    let module = &body[mstart..mend];
                    o = mend;
                    let (nlen, no) = read_leb(&body[o..]);
                    let nstart = o + no;
                    let nend = nstart + nlen as usize;
                    if nend > body.len() {
                        return false;
                    }
                    let name = &body[nstart..nend];
                    o = nend + 1; // import kind
                    let (_t, to) = read_leb(&body[o..]);
                    o += to;
                    if module == b"wasi_snapshot_preview1" && name == b"proc_exit" {
                        return true;
                    }
                }
                return false;
            }
            i = end;
        }
        false
    }

    #[test]
    fn exports_wasi_start_and_proc_exit_import() {
        let data = compile("fn main() -> i32 { 42 }");
        assert!(has_export(&data, "_start"), "module must export _start");
        assert!(has_export(&data, "main"), "module must still export main");
        assert!(has_export(&data, "memory"), "module must export memory");
        assert!(
            has_wasi_proc_exit_import(&data),
            "module must import wasi proc_exit"
        );
    }

    #[test]
    fn void_main_gets_zero_exit_path() {
        let data = compile("fn main() { }");
        assert!(has_export(&data, "_start"));
        assert!(has_wasi_proc_exit_import(&data));
    }

    #[test]
    fn module_without_main_has_no_start() {
        let data = compile("fn helper() -> i32 { 7 }");
        assert!(!has_export(&data, "_start"), "no main means no _start");
    }

    #[test]
    fn recursion_module_is_emitted() {
        let data = compile(
            "fn fib(n: i32) -> i32 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } } fn main() -> i32 { fib(10) }",
        );
        assert!(has_export(&data, "fib"));
        assert!(has_export(&data, "_start"));
    }

    /// `i32.sub` pops (top) last, so `0 - a` requires pushing the zero before
    /// the operand. The reversed order silently produced `a - 0`, making
    /// unary minus a no-op for every negative literal.
    #[test]
    fn negation_pushes_zero_before_operand() {
        let lir = build_lir("fn main() -> i32 { -1 }");
        let data = emit_module_bytes(&lir).unwrap();
        // main is the first defined function; find its body.
        let body = first_code_body(&data).expect("main body");
        // local.get 2, i32.const 0, i32.sub  == wrong order
        let wrong = [&[0x20, 0x02][..], &[0x41, 0x00][..], &[0x6b][..]].concat();
        // i32.const 0, local.get 2, i32.sub  == correct order
        let right = [&[0x41, 0x00][..], &[0x20, 0x02][..], &[0x6b][..]].concat();
        assert!(
            contains(&body, &right),
            "expected i32.const 0 then local.get 2 then i32.sub"
        );
        assert!(
            !contains(&body, &wrong),
            "operand must not be pushed before the zero constant"
        );
    }

    /// A struct value is a pointer into the shadow stack, and the callee resets
    /// that stack on return. The caller has to allocate its own copy, so
    /// `main` must contain a stack-pointer bump for the aggregate it receives.
    /// `main` is declared first so it is the first entry in the code section.
    #[test]
    fn caller_reallocates_returned_aggregate() {
        let lir = build_lir(
            "struct P { x: i32 }\n\
             fn main() -> i32 { let a = make(1); a.x }\n\
             fn make(v: i32) -> P { P { x: v } }",
        );
        let data = emit_module_bytes(&lir).unwrap();
        let body = first_code_body(&data).expect("main body");
        // global.get 0, i32.const 4, i32.sub == reserving four shadow-stack bytes
        let stack_bump = [&[0x23, 0x00][..], &[0x41, 0x04][..], &[0x6b][..]].concat();
        assert!(
            contains(&body, &stack_bump),
            "main must copy the returned struct into its own stack slot"
        );
    }

    /// An enum discriminant is a payload field, so it has to be materialised
    /// into a local before StructInit reads it. Passing the literal straight
    /// through made every variant collapse to register 0.
    #[test]
    fn enum_init_stores_the_tag_before_building_the_value() {
        let lir =
            build_lir("enum Color { Red, Green } fn main() -> i32 { let c = Color.Green; 0 }");
        let data = emit_module_bytes(&lir).unwrap();
        let body = first_code_body(&data).expect("main body");
        // i32.const 1 must appear as a plain local.set before the struct store,
        // which is the discriminant being parked in its own register.
        let tag_local_set = [&[0x41, 0x01][..], &[0x21][..]].concat();
        assert!(
            contains(&body, &tag_local_set),
            "the variant tag must be written to a local before StructInit"
        );
    }
}
