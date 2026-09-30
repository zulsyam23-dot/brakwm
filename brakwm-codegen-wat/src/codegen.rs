use brakwm_codegen_traits::CodegenBackend;
use brakwm_core::Result;
use brakwm_ir_lir::lir::{BlockId, LirOpcode, LirOperand, LirProgram, LirType, VirtReg};

use crate::common::*;

/// Append an indented line to the output buffer.
macro_rules! emit {
    ($out:expr, $($t:tt)*) => {{
        $out.push_str("    ");
        $out.push_str(&format!($($t)*));
        $out.push('\n');
    }};
}

/// WASM backend emitting WAT text.
pub struct WatCodegen;

impl WatCodegen {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WatCodegen {
    fn default() -> Self {
        Self::new()
    }
}

impl CodegenBackend for WatCodegen {
    fn name(&self) -> &'static str {
        "wat"
    }

    fn emit(&self, program: &LirProgram) -> Result<Vec<u8>> {
        let wat = emit_module(program)?;
        Ok(wat.into_bytes())
    }
}

/// Return WAT text for the entire module.
pub fn emit_module(program: &LirProgram) -> Result<String> {
    let mut out = String::new();
    let string_addrs = layout_strings(program);

    out.push_str("(module\n");
    out.push_str(&format!(
        "  (memory (export \"memory\") {min})\n",
        min = MIN_PAGES
    ));
    out.push_str(&format!(
        "  (global $__brakwm_sp (mut i32) (i32.const {base}))\n",
        base = STACK_BASE
    ));

    for e in &program.extern_functions {
        let (namespace, name) = extern_namespace(&e.name);
        let params: Vec<&str> = e.params.iter().map(wasm_type).collect();
        let ret = match wasm_ret(&e.ret_ty) {
            Some(r) => format!(" (result {r})"),
            None => String::new(),
        };
        out.push_str(&format!(
            "  (import \"{namespace}\" \"{name}\" (func ${f} (param {p}){r}))\n",
            f = e.name,
            p = params.join(" "),
            r = ret
        ));
    }

    for (i, s) in program.string_table.iter().enumerate() {
        out.push_str(&format!(
            "  (data (i32.const {addr}) \"{esc}\")\n",
            addr = string_addrs[i],
            esc = escape_string(s)
        ));
    }

    for func in &program.functions {
        out.push_str(&emit_function(func, program));
    }

    out.push_str(")\n");
    Ok(out)
}

fn wasm_ret(t: &LirType) -> Option<&'static str> {
    if matches!(t, LirType::Void) {
        None
    } else {
        Some(wasm_type(t))
    }
}

fn extern_namespace(name: &str) -> (&'static str, String) {
    if name == "proc_exit" {
        ("wasi_snapshot_preview1", "proc_exit".to_string())
    } else {
        ("env", name.to_string())
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

fn escape_string(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\{:02x}", b)),
        }
    }
    out
}

/// Emit one function. Registers map 1:1 to WAT params+locals. The CFG is
/// translated with a cascade of `block` scopes + one `br_table` dispatcher,
/// which is correct for any CFG (reducible or not).
fn emit_function(func: &brakwm_ir_lir::lir::LirFunction, program: &LirProgram) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  (func ${} (export \"{}\")",
        func.name, func.name
    ));

    for p in &func.params {
        out.push_str(&format!(
            " (param {} {})",
            reg_name(*p),
            wasm_type(&func.reg_types[*p])
        ));
    }
    if let Some(r) = wasm_ret(&func.ret_ty) {
        out.push_str(&format!(" (result {r})"));
    }
    out.push('\n');

    for r in 0..func.reg_types.len() {
        if func.params.contains(&r) {
            continue;
        }
        out.push_str(&format!(
            "    (local {} {})\n",
            reg_name(r),
            wasm_type(&func.reg_types[r])
        ));
    }
    out.push_str("    (local $__pc i32)\n    (local $__sp_save i32)\n");

    let uses_stack = func.blocks.iter().any(|b| {
        b.insts
            .iter()
            .any(|i| matches!(i.opcode, LirOpcode::Alloca | LirOpcode::StructInit))
    });

    // Entry: jump into dispatch. The loop keeps re-reading $__pc.
    out.push_str("    (local.set $__pc (i32.const 0))\n");
    if uses_stack {
        out.push_str("    (local.set $__sp_save (global.get $__brakwm_sp))\n");
    }

    out.push_str(&emit_dispatch_body(func, program, uses_stack));
    out.push_str("  )\n\n");
    out
}

fn emit_dispatch_body(
    func: &brakwm_ir_lir::lir::LirFunction,
    program: &LirProgram,
    uses_stack: bool,
) -> String {
    let n = func.blocks.len();
    let mut out = String::new();

    // Cascade dispatcher: labels $__b0..$__b{n-1}, b0 outermost, b{n-1}
    // innermost (contains the br_table). Body i is placed immediately after
    // `end $__b{i}`, so `br $__b{i}` lands exactly at body i.
    out.push_str("    (loop $__dispatch\n");
    let mut indent = "      ".to_string();
    for i in 0..n {
        out.push_str(&format!("{indent}(block $__b{i}\n"));
        indent.push_str("  ");
    }
    let mut table = String::from("(br_table");
    for i in 0..n {
        table.push_str(&format!(" $__b{i}"));
    }
    table.push_str(" (local.get $__pc))");
    out.push_str(&format!("{indent}{table}\n"));

    // Close from innermost: end b{i}, body i, dispatch.
    for i in (0..n).rev() {
        indent.truncate(indent.len() - 2);
        out.push_str(&format!("{indent}) ;; end $__b{i}\n"));
        out.push_str(&emit_block_insts(func, program, i, uses_stack));
        out.push_str(&format!("{indent}(br $__dispatch)\n"));
    }
    out.push_str("    ) ;; loop $__dispatch\n");
    out
}

/// Emit the instructions of a single block. Control transfer (Jmp/Br/Ret) is
/// turned into `$__pc` updates + `br $__dispatch` (or `return`).
fn emit_block_insts(
    func: &brakwm_ir_lir::lir::LirFunction,
    program: &LirProgram,
    bi: usize,
    uses_stack: bool,
) -> String {
    let mut out = String::new();
    let block = &func.blocks[bi];
    let mut pending_cmp: Option<(VirtReg, VirtReg)> = None;

    for inst in &block.insts {
        match inst.opcode {
            LirOpcode::Comment => {}
            LirOpcode::Mov => {
                let d = inst.dest.unwrap();
                match &inst.operands[0] {
                    LirOperand::ImmI64(v) => {
                        let op = const_op(wasm_type(&func.reg_types[d]));
                        emit!(out, "(local.set {} ({} {}))", reg_name(d), op, v);
                    }
                    LirOperand::ImmF64(v) => {
                        let op = const_op(wasm_type(&func.reg_types[d]));
                        emit!(
                            out,
                            "(local.set {} ({} {}))",
                            reg_name(d),
                            op,
                            fmt_float(*v)
                        );
                    }
                    LirOperand::Reg(s) => {
                        emit!(
                            out,
                            "(local.set {} (local.get {}))",
                            reg_name(d),
                            reg_name(*s)
                        );
                    }
                    LirOperand::StringRef(i) => {
                        let addr = layout_strings(program)
                            .get(*i)
                            .copied()
                            .unwrap_or(DATA_BASE);
                        emit!(out, "(local.set {} (i32.const {addr}))", reg_name(d));
                    }
                    _ => unreachable!("unexpected Mov operand"),
                }
            }
            LirOpcode::Add | LirOpcode::Sub | LirOpcode::Mul | LirOpcode::Div | LirOpcode::Mod => {
                let d = inst.dest.unwrap();
                let a = reg_of(&inst.operands[0]);
                let b = reg_of(&inst.operands[1]);
                let op = match inst.opcode {
                    LirOpcode::Add => "add",
                    LirOpcode::Sub => "sub",
                    LirOpcode::Mul => "mul",
                    LirOpcode::Div => "div_s",
                    LirOpcode::Mod => "rem_s",
                    _ => unreachable!(),
                };
                let t = wasm_type(&func.reg_types[d]);
                emit!(
                    out,
                    "(local.set {} ({} {} (local.get {}) (local.get {})))",
                    reg_name(d),
                    t,
                    op,
                    reg_name(a),
                    reg_name(b)
                );
            }
            LirOpcode::FAdd | LirOpcode::FSub | LirOpcode::FMul | LirOpcode::FDiv => {
                let d = inst.dest.unwrap();
                let a = reg_of(&inst.operands[0]);
                let b = reg_of(&inst.operands[1]);
                let op = match inst.opcode {
                    LirOpcode::FAdd => "add",
                    LirOpcode::FSub => "sub",
                    LirOpcode::FMul => "mul",
                    LirOpcode::FDiv => "div",
                    _ => unreachable!(),
                };
                let t = wasm_type(&func.reg_types[d]);
                emit!(
                    out,
                    "(local.set {} ({} {} (local.get {}) (local.get {})))",
                    reg_name(d),
                    t,
                    op,
                    reg_name(a),
                    reg_name(b)
                );
            }
            LirOpcode::Neg => {
                let d = inst.dest.unwrap();
                let a = reg_of(&inst.operands[0]);
                let t = wasm_type(&func.reg_types[d]);
                match t {
                    "f32" | "f64" => {
                        emit!(
                            out,
                            "(local.set {} ({} neg (local.get {})))",
                            reg_name(d),
                            t,
                            reg_name(a)
                        );
                    }
                    _ => {
                        let op = const_op(t);
                        emit!(
                            out,
                            "(local.set {} ({} sub ({} 0) (local.get {})))",
                            reg_name(d),
                            t,
                            op,
                            reg_name(a)
                        );
                    }
                }
            }
            LirOpcode::Not => {
                let d = inst.dest.unwrap();
                let a = reg_of(&inst.operands[0]);
                emit!(
                    out,
                    "(local.set {} (i32.eqz (local.get {})))",
                    reg_name(d),
                    reg_name(a)
                );
            }
            LirOpcode::And | LirOpcode::Or | LirOpcode::Xor | LirOpcode::Shl | LirOpcode::Shr => {
                let d = inst.dest.unwrap();
                let a = reg_of(&inst.operands[0]);
                let b = reg_of(&inst.operands[1]);
                let op = match inst.opcode {
                    LirOpcode::And => "and",
                    LirOpcode::Or => "or",
                    LirOpcode::Xor => "xor",
                    LirOpcode::Shl => "shl",
                    LirOpcode::Shr => "shr_s",
                    _ => unreachable!(),
                };
                let t = wasm_type(&func.reg_types[d]);
                emit!(
                    out,
                    "(local.set {} ({} {} (local.get {}) (local.get {})))",
                    reg_name(d),
                    t,
                    op,
                    reg_name(a),
                    reg_name(b)
                );
            }
            LirOpcode::Cmp => {
                let a = reg_of(&inst.operands[0]);
                let b = reg_of(&inst.operands[1]);
                pending_cmp = Some((a, b));
            }
            LirOpcode::SetEq
            | LirOpcode::SetNe
            | LirOpcode::SetLt
            | LirOpcode::SetLe
            | LirOpcode::SetGt
            | LirOpcode::SetGe => {
                let d = inst.dest.unwrap();
                let (a, b) = pending_cmp.take().unwrap_or((0, 0));
                let at = wasm_type(&func.reg_types[a]);
                let rel = match inst.opcode {
                    LirOpcode::SetEq => "eq",
                    LirOpcode::SetNe => "ne",
                    LirOpcode::SetLt => "lt",
                    LirOpcode::SetLe => "le",
                    LirOpcode::SetGt => "gt",
                    LirOpcode::SetGe => "ge",
                    _ => unreachable!(),
                };
                let signed = if at == "f32" || at == "f64" {
                    String::new()
                } else {
                    "_s".to_string()
                };
                emit!(
                    out,
                    "(local.set {} (i32.{}{} (local.get {}) (local.get {})))",
                    reg_name(d),
                    rel,
                    signed,
                    reg_name(a),
                    reg_name(b)
                );
            }
            LirOpcode::Load => {
                let d = inst.dest.unwrap();
                let a = reg_of(&inst.operands[0]);
                let t = wasm_type(&func.reg_types[d]);
                emit!(
                    out,
                    "(local.set {} ({} load (local.get {})))",
                    reg_name(d),
                    t,
                    reg_name(a)
                );
            }
            LirOpcode::Store => {
                let addr = reg_of(&inst.operands[0]);
                let val = reg_of(&inst.operands[1]);
                let t = wasm_type(&func.reg_types[val]);
                emit!(
                    out,
                    "({}.store (local.get {}) (local.get {}))",
                    t,
                    reg_name(addr),
                    reg_name(val)
                );
            }
            LirOpcode::Alloca => {
                let d = inst.dest.unwrap();
                let size = match inst.operands.first() {
                    Some(LirOperand::ImmI64(v)) => align4(*v as u32),
                    _ => 16,
                };
                emit!(
                    out,
                    "(local.set {} (global.get $__brakwm_sp)) (global.set $__brakwm_sp (i32.sub (global.get $__brakwm_sp) (i32.const {size})))",
                    reg_name(d)
                );
            }
            LirOpcode::StructInit => {
                let d = inst.dest.unwrap();
                let name = label_of(&inst.operands[0]);
                let fields = struct_fields(program, &name);
                let size = align4(struct_size(program, &name));
                emit!(
                    out,
                    "(local.set {} (global.get $__brakwm_sp)) (global.set $__brakwm_sp (i32.sub (global.get $__brakwm_sp) (i32.const {size})))",
                    reg_name(d)
                );
                let mut i = 1;
                while i + 1 < inst.operands.len() {
                    let fname = field_of(&inst.operands[i]);
                    let val = reg_of(&inst.operands[i + 1]);
                    let off = fields.get(&fname).copied().unwrap_or(0);
                    let ft = field_ty(program, &name, &fname)
                        .unwrap_or_else(|| func.reg_types[val].clone());
                    let t = wasm_type(&ft);
                    emit!(
                        out,
                        "({}.store (i32.add (local.get {}) (i32.const {off})) (local.get {}))",
                        t,
                        reg_name(d),
                        reg_name(val)
                    );
                    i += 2;
                }
            }
            LirOpcode::GetField => {
                let d = inst.dest.unwrap();
                let obj = reg_of(&inst.operands[0]);
                let fname = field_of(&inst.operands[1]);
                let name = label_of(&inst.operands[2]);
                let fields = struct_fields(program, &name);
                let off = fields.get(&fname).copied().unwrap_or(0);
                let ft =
                    field_ty(program, &name, &fname).unwrap_or_else(|| func.reg_types[d].clone());
                let t = wasm_type(&ft);
                emit!(
                    out,
                    "(local.set {} ({} load (i32.add (local.get {}) (i32.const {off}))))",
                    reg_name(d),
                    t,
                    reg_name(obj)
                );
            }
            LirOpcode::SetField => {
                let obj = reg_of(&inst.operands[0]);
                let fname = field_of(&inst.operands[1]);
                let val = reg_of(&inst.operands[2]);
                let name = label_of(&inst.operands[3]);
                let fields = struct_fields(program, &name);
                let off = fields.get(&fname).copied().unwrap_or(0);
                let ft =
                    field_ty(program, &name, &fname).unwrap_or_else(|| func.reg_types[val].clone());
                let t = wasm_type(&ft);
                emit!(
                    out,
                    "({}.store (i32.add (local.get {}) (i32.const {off})) (local.get {}))",
                    t,
                    reg_name(obj),
                    reg_name(val)
                );
            }
            LirOpcode::Call => {
                let callee = label_of(&inst.operands[0]);
                // Skip over operands that precede args (operand[0] is the label).
                let args: Vec<VirtReg> = inst
                    .operands
                    .iter()
                    .skip(1)
                    .filter_map(reg_of_opt)
                    .collect();
                let mut call = format!("(call ${callee}");
                for a in &args {
                    call.push_str(&format!(" (local.get {})", reg_name(*a)));
                }
                call.push(')');
                if let Some(d) = inst.dest {
                    emit!(out, "(local.set {} {})", reg_name(d), call);
                } else {
                    emit!(out, "{}", call);
                }
            }
            LirOpcode::Ret => {
                // Restore the shadow stack only *after* reading the return
                // value out of the local. Restoring first is fine for scalars,
                // but a returned struct points into the stack, and resetting
                // the stack pointer here freed that memory before the caller
                // could read it.
                let ret_operand = match inst.operands.first() {
                    Some(LirOperand::Reg(r)) => Some(*r),
                    _ => None,
                };
                let val = match ret_operand {
                    Some(r) => format!("(local.get {})", reg_name(r)),
                    None => String::new(),
                };
                if uses_stack {
                    out.push_str("    (global.set $__brakwm_sp (local.get $__sp_save)) ");
                }
                if ret_operand.is_some() {
                    out.push_str(&format!("(return {val})\n"));
                } else {
                    out.push_str("(return)\n");
                }
            }
            LirOpcode::Jmp => {
                let target = label_of(&inst.operands[0]);
                let idx = block_index_by_ident(func, &target);
                emit!(out, "(local.set $__pc (i32.const {idx})) (br $__dispatch)");
            }
            LirOpcode::Br => {
                let cond = reg_of(&inst.operands[0]);
                let then_t = label_of(&inst.operands[1]);
                let else_t = label_of(&inst.operands[2]);
                let then_idx = block_index_by_ident(func, &then_t);
                let else_idx = block_index_by_ident(func, &else_t);
                emit!(
                    out,
                    "(if (local.get {}) (then (local.set $__pc (i32.const {then_idx}))) (else (local.set $__pc (i32.const {else_idx})))) (br $__dispatch)",
                    reg_name(cond)
                );
            }
            LirOpcode::Push | LirOpcode::Pop => {
                // No-op in WASM model (stack is implicit).
            }
        }
    }

    out
}

fn block_index_by_ident(func: &brakwm_ir_lir::lir::LirFunction, label: &str) -> usize {
    let id: BlockId = label
        .strip_prefix("block_")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    func.blocks.iter().position(|b| b.id == id).unwrap_or(0)
}

// -- helpers ---------------------------------------------------------------

fn reg_of(op: &LirOperand) -> VirtReg {
    match op {
        LirOperand::Reg(r) => *r,
        _ => 0,
    }
}

fn reg_of_opt(op: &LirOperand) -> Option<VirtReg> {
    match op {
        LirOperand::Reg(r) => Some(*r),
        _ => None,
    }
}

fn label_of(op: &LirOperand) -> String {
    match op {
        LirOperand::Label(s) => s.clone(),
        _ => String::new(),
    }
}

fn field_of(op: &LirOperand) -> String {
    match op {
        LirOperand::Field(s) => s.clone(),
        _ => String::new(),
    }
}

fn const_op(t: &str) -> &'static str {
    match t {
        "f32" => "f32.const",
        "f64" => "f64.const",
        "i64" => "i64.const",
        _ => "i32.const",
    }
}

fn fmt_float(f: f64) -> String {
    if f == f.trunc() && f.abs() < 1e15 {
        format!("{:.1}", f)
    } else {
        format!("{}", f)
    }
}

fn type_size(t: &LirType) -> u32 {
    match t {
        LirType::I64 | LirType::F64 => 8,
        _ => 4,
    }
}

fn struct_fields(program: &LirProgram, name: &str) -> std::collections::HashMap<String, u32> {
    let mut map = std::collections::HashMap::new();
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

#[cfg(test)]
mod tests {
    use super::*;
    use brakwm_ir_lir::lower::LirLower;

    fn lir_of(src: &str) -> LirProgram {
        let sm = brakwm_core::SourceMap::new("t.brk", src);
        let mut p = brakwm_frontend::parser::Parser::new();
        let ast = p
            .parse_source(&sm)
            .unwrap_or_else(|e| panic!("parse failed for {src:?}: {e}"));
        let hir = brakwm_ir_hir::lower::HirLower::new().lower(ast).unwrap();
        let mut ml = brakwm_ir_mir::lower::MirLower::new();
        let mir = ml.lower(hir).unwrap();
        let mut ll = LirLower::new();
        ll.set_file_id(0);
        ll.lower(mir)
    }

    #[test]
    fn emits_hello_module() {
        let p = lir_of("fn main() -> i32 { 42 }");
        let wat = emit_module(&p).unwrap();
        assert!(wat.contains("(module"));
        assert!(wat.contains("(func $main"));
        assert!(wat.contains("\"main\""));
        assert!(wat.contains("i32.const 42"));
    }

    #[test]
    fn emits_branch_module() {
        let p = lir_of("fn main() -> i32 { let x = if 3 > 2 { 10 } else { 20 }; x }");
        let wat = emit_module(&p).unwrap();
        assert!(wat.contains("br_table"));
        assert!(wat.contains("(export \"main\""));
    }

    #[test]
    fn emits_loop_module() {
        let p = lir_of("fn main() -> i32 { let mut i = 0; while i < 5 { i = i + 1 } i }");
        let wat = emit_module(&p).unwrap();
        assert!(wat.contains("br_table"));
    }
}
