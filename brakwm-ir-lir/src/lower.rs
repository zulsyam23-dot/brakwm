use brakwm_core::Span;
use brakwm_ir_mir::mir::*;
use std::collections::HashMap;

use crate::lir::*;

pub struct LirLower {
    externs: HashMap<String, (CallingConvention, Vec<LirType>, LirType)>,
    /// enum name -> variant name -> tag index.
    enum_tags: HashMap<String, HashMap<String, i64>>,
    string_table: Vec<String>,
    current_file_id: usize,
}

impl Default for LirLower {
    fn default() -> Self {
        Self::new()
    }
}

fn lower_mir_type_to_lir(ty: &MirType) -> LirType {
    match ty {
        MirType::I32 => LirType::I32,
        MirType::I64 => LirType::I64,
        MirType::F32 => LirType::F32,
        MirType::F64 => LirType::F64,
        MirType::Bool => LirType::Bool,
        MirType::String => LirType::String,
        MirType::Void => LirType::Void,
        MirType::Named(s) => LirType::Named(s.clone()),
    }
}

impl LirLower {
    pub fn new() -> Self {
        Self {
            externs: HashMap::new(),
            enum_tags: HashMap::new(),
            string_table: Vec::new(),
            current_file_id: 0,
        }
    }

    pub fn set_file_id(&mut self, file_id: usize) {
        self.current_file_id = file_id;
    }

    fn intern_string(&mut self, s: &str) -> usize {
        if let Some(i) = self.string_table.iter().position(|x| x == s) {
            i
        } else {
            let i = self.string_table.len();
            self.string_table.push(s.to_string());
            i
        }
    }

    pub fn lower(&mut self, program: MirProgram) -> LirProgram {
        let mut structs: Vec<LirStructMetadata> = program
            .structs
            .iter()
            .map(|s| LirStructMetadata {
                name: s.name.clone(),
                fields: s
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), lower_mir_type_to_lir(&f.ty)))
                    .collect(),
            })
            .collect();

        // Synthesize aggregate metadata for enums as `__enum_<Name>` structs
        // so GetField/SetField offsets resolve in backends.
        for e in &program.enums {
            let max_payload = e
                .variants
                .iter()
                .map(|v| v.fields.as_ref().map(|f| f.len()).unwrap_or(0))
                .max()
                .unwrap_or(0);
            let mut fields = vec![("$tag".to_string(), LirType::I32)];
            for i in 0..max_payload {
                fields.push((format!("${i}"), LirType::I32));
            }
            structs.push(LirStructMetadata {
                name: format!("__enum_{}", e.name),
                fields,
            });
        }

        let enums = program
            .enums
            .iter()
            .map(|e| {
                for (i, v) in e.variants.iter().enumerate() {
                    self.enum_tags
                        .entry(e.name.clone())
                        .or_default()
                        .insert(v.name.clone(), i as i64);
                }
                LirEnumMetadata {
                    name: e.name.clone(),
                    variants: e
                        .variants
                        .iter()
                        .map(|v| {
                            (
                                v.name.clone(),
                                v.fields
                                    .as_ref()
                                    .map(|fs| fs.iter().map(lower_mir_type_to_lir).collect()),
                            )
                        })
                        .collect(),
                }
            })
            .collect();

        let extern_functions: Vec<LirExternFunction> = program
            .extern_functions
            .into_iter()
            .map(|e| {
                let cc = match e.abi.to_lowercase().as_str() {
                    "c" | "cdecl" => CallingConvention::Cdecl,
                    "win64" => CallingConvention::Win64,
                    "systemv" => CallingConvention::SystemV,
                    _ => CallingConvention::Brak,
                };
                let params: Vec<LirType> = e.params.iter().map(lower_mir_type_to_lir).collect();
                let ret_ty = lower_mir_type_to_lir(&e.ret_ty);
                self.externs
                    .insert(e.name.clone(), (cc, params.clone(), ret_ty.clone()));
                LirExternFunction {
                    name: e.name,
                    abi: cc,
                    params,
                    ret_ty,
                    span: e.span,
                }
            })
            .collect();

        let functions = program
            .functions
            .into_iter()
            .map(|f| self.lower_function(f))
            .collect();

        let string_table = std::mem::take(&mut self.string_table);
        LirProgram {
            functions,
            extern_functions,
            structs,
            enums,
            string_table,
            files: vec![],
        }
    }

    fn lower_function(&mut self, func: MirFunction) -> LirFunction {
        // reg_types spans every local: params come first, then temps.
        let mut reg_types: Vec<LirType> = func
            .locals
            .iter()
            .map(|l| lower_mir_type_to_lir(&l.ty))
            .collect();
        reg_types.resize(func.locals.len(), LirType::I32);

        let blocks: Vec<LirBlock> = func
            .blocks
            .iter()
            .map(|b| self.lower_block(b.clone()))
            .collect();

        let ret_ty = lower_mir_type_to_lir(&func.ret_ty);
        LirFunction {
            name: func.name,
            params: func.params,
            ret_ty,
            reg_types,
            reg_count: func.locals.len() + 1,
            blocks,
            span: func.span,
        }
    }

    fn lower_block(&mut self, block: MirBlock) -> LirBlock {
        let mut insts = vec![];

        for inst in &block.insts {
            match inst {
                MirInst::Assign { dest, value, span } => {
                    self.lower_assign(*dest, value, *span, &mut insts);
                }
                MirInst::Call {
                    dest,
                    callee,
                    args,
                    span,
                } => {
                    let mut lir = LirInst::new(LirOpcode::Call)
                        .with_op(LirOperand::Label(callee.clone()))
                        .with_debug(*span);

                    if let Some((cc, _, _)) = self.externs.get(callee) {
                        lir = lir.with_call_conv(*cc);
                    }

                    for arg in args {
                        lir = lir.with_op(LirOperand::Reg(*arg));
                    }
                    if let Some(d) = dest {
                        lir = lir.with_dest(*d);
                    }
                    insts.push(lir);
                }
            }
        }

        match &block.terminator {
            MirTerminator::Return { value, span } => {
                if let Some(v) = value {
                    insts.push(
                        LirInst::new(LirOpcode::Ret)
                            .with_op(LirOperand::Reg(*v))
                            .with_debug(*span),
                    );
                } else {
                    insts.push(LirInst::new(LirOpcode::Ret).with_debug(*span));
                }
            }
            MirTerminator::Jump { target, span } => {
                insts.push(
                    LirInst::new(LirOpcode::Jmp)
                        .with_op(LirOperand::Label(format!("block_{target}")))
                        .with_debug(*span),
                );
            }
            MirTerminator::Branch {
                cond,
                then,
                else_,
                span,
            } => {
                insts.push(
                    LirInst::new(LirOpcode::Br)
                        .with_op(LirOperand::Reg(*cond))
                        .with_op(LirOperand::Label(format!("block_{then}")))
                        .with_op(LirOperand::Label(format!("block_{else_}")))
                        .with_debug(*span),
                );
            }
            MirTerminator::Unreachable => {}
        }

        for inst in &mut insts {
            inst.file_id = self.current_file_id;
        }

        LirBlock {
            id: block.id,
            name: block.name,
            insts,
            span: block.span,
        }
    }

    fn lower_assign(
        &mut self,
        dest: usize,
        value: &MirValue,
        span: Span,
        insts: &mut Vec<LirInst>,
    ) {
        match value {
            MirValue::Local(src) => {
                insts.push(
                    LirInst::new(LirOpcode::Mov)
                        .with_dest(dest)
                        .with_op(LirOperand::Reg(*src))
                        .with_debug(span),
                );
            }
            MirValue::Int(i) => {
                insts.push(
                    LirInst::new(LirOpcode::Mov)
                        .with_dest(dest)
                        .with_op(LirOperand::ImmI64(*i))
                        .with_debug(span),
                );
            }
            MirValue::Float(f) => {
                insts.push(
                    LirInst::new(LirOpcode::Mov)
                        .with_dest(dest)
                        .with_op(LirOperand::ImmF64(*f))
                        .with_debug(span),
                );
            }
            MirValue::Bool(b) => {
                insts.push(
                    LirInst::new(LirOpcode::Mov)
                        .with_dest(dest)
                        .with_op(LirOperand::ImmI64(if *b { 1 } else { 0 }))
                        .with_debug(span),
                );
            }
            MirValue::String(s) => {
                let idx = self.intern_string(s);
                insts.push(
                    LirInst::new(LirOpcode::Mov)
                        .with_dest(dest)
                        .with_op(LirOperand::StringRef(idx))
                        .with_debug(span),
                );
            }
            MirValue::BinOp { op, lhs, rhs } => match op {
                MirBinOp::FAdd | MirBinOp::FSub | MirBinOp::FMul | MirBinOp::FDiv => {
                    let lir_op = match op {
                        MirBinOp::FAdd => LirOpcode::FAdd,
                        MirBinOp::FSub => LirOpcode::FSub,
                        MirBinOp::FMul => LirOpcode::FMul,
                        MirBinOp::FDiv => LirOpcode::FDiv,
                        _ => unreachable!(),
                    };
                    insts.push(
                        LirInst::new(lir_op)
                            .with_dest(dest)
                            .with_op(LirOperand::Reg(*lhs))
                            .with_op(LirOperand::Reg(*rhs))
                            .with_debug(span),
                    );
                }
                MirBinOp::Add
                | MirBinOp::Sub
                | MirBinOp::Mul
                | MirBinOp::Div
                | MirBinOp::Mod
                | MirBinOp::And
                | MirBinOp::Or
                | MirBinOp::BitAnd
                | MirBinOp::BitOr
                | MirBinOp::BitXor
                | MirBinOp::Shl
                | MirBinOp::Shr => {
                    let lir_op = match op {
                        MirBinOp::Add => LirOpcode::Add,
                        MirBinOp::Sub => LirOpcode::Sub,
                        MirBinOp::Mul => LirOpcode::Mul,
                        MirBinOp::Div => LirOpcode::Div,
                        MirBinOp::Mod => LirOpcode::Mod,
                        MirBinOp::And | MirBinOp::BitAnd => LirOpcode::And,
                        MirBinOp::Or | MirBinOp::BitOr => LirOpcode::Or,
                        MirBinOp::BitXor => LirOpcode::Xor,
                        MirBinOp::Shl => LirOpcode::Shl,
                        MirBinOp::Shr => LirOpcode::Shr,
                        _ => unreachable!(),
                    };
                    insts.push(
                        LirInst::new(lir_op)
                            .with_dest(dest)
                            .with_op(LirOperand::Reg(*lhs))
                            .with_op(LirOperand::Reg(*rhs))
                            .with_debug(span),
                    );
                }
                MirBinOp::Eq
                | MirBinOp::Ne
                | MirBinOp::Lt
                | MirBinOp::Le
                | MirBinOp::Gt
                | MirBinOp::Ge => {
                    let set_op = match op {
                        MirBinOp::Eq => LirOpcode::SetEq,
                        MirBinOp::Ne => LirOpcode::SetNe,
                        MirBinOp::Lt => LirOpcode::SetLt,
                        MirBinOp::Le => LirOpcode::SetLe,
                        MirBinOp::Gt => LirOpcode::SetGt,
                        MirBinOp::Ge => LirOpcode::SetGe,
                        _ => unreachable!(),
                    };
                    insts.push(
                        LirInst::new(LirOpcode::Cmp)
                            .with_op(LirOperand::Reg(*lhs))
                            .with_op(LirOperand::Reg(*rhs))
                            .with_debug(span),
                    );
                    insts.push(LirInst::new(set_op).with_dest(dest).with_debug(span));
                }
            },
            MirValue::UnOp { op, expr } => {
                if let MirUnOp::BitNot = op {
                    insts.push(
                        LirInst::new(LirOpcode::Mov)
                            .with_dest(dest)
                            .with_op(LirOperand::ImmI64(-1))
                            .with_debug(span),
                    );
                    insts.push(
                        LirInst::new(LirOpcode::Xor)
                            .with_dest(dest)
                            .with_op(LirOperand::Reg(*expr))
                            .with_op(LirOperand::Reg(dest))
                            .with_debug(span),
                    );
                } else {
                    let lir_op = match op {
                        MirUnOp::Neg => LirOpcode::Neg,
                        MirUnOp::Not => LirOpcode::Not,
                        MirUnOp::BitNot => unreachable!("handled above"),
                    };
                    insts.push(
                        LirInst::new(lir_op)
                            .with_dest(dest)
                            .with_op(LirOperand::Reg(*expr))
                            .with_debug(span),
                    );
                }
            }
            MirValue::GetField {
                object,
                name,
                field,
            } => {
                insts.push(
                    LirInst::new(LirOpcode::GetField)
                        .with_dest(dest)
                        .with_op(LirOperand::Reg(*object))
                        .with_op(LirOperand::Field(field.clone()))
                        .with_op(LirOperand::Label(name.clone()))
                        .with_debug(span),
                );
            }
            MirValue::StructInit { name, fields } => {
                let mut lir = LirInst::new(LirOpcode::StructInit)
                    .with_dest(dest)
                    .with_op(LirOperand::Label(name.clone()))
                    .with_debug(span);
                for (fname, fval) in fields {
                    lir = lir.with_op(LirOperand::Field(fname.clone()));
                    lir = lir.with_op(LirOperand::Reg(*fval));
                }
                insts.push(lir);
            }
            MirValue::EnumInit {
                enum_name,
                variant: _,
                args,
            } => {
                // Enums are a synthetic struct `__enum_<Name>` laid out as
                // [$tag, $0, $1, ...]. MIR passes the tag as the first payload
                // local, because StructInit can only store from a register.
                let mut lir = LirInst::new(LirOpcode::StructInit)
                    .with_dest(dest)
                    .with_op(LirOperand::Label(format!("__enum_{}", enum_name)))
                    .with_debug(span);
                lir = lir.with_op(LirOperand::Field("$tag".into()));
                if let Some(tag_reg) = args.first() {
                    lir = lir.with_op(LirOperand::Reg(*tag_reg));
                } else {
                    lir = lir.with_op(LirOperand::ImmI64(0));
                }
                for (i, a) in args.iter().skip(1).enumerate() {
                    lir = lir.with_op(LirOperand::Field(format!("${i}")));
                    lir = lir.with_op(LirOperand::Reg(*a));
                }
                insts.push(lir);
            }
            MirValue::SetField {
                object,
                name,
                field,
                value,
            } => {
                insts.push(
                    LirInst::new(LirOpcode::SetField)
                        .with_dest(dest)
                        .with_op(LirOperand::Reg(*object))
                        .with_op(LirOperand::Field(field.clone()))
                        .with_op(LirOperand::Reg(*value))
                        .with_op(LirOperand::Label(name.clone()))
                        .with_debug(span),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brakwm_core::SourceMap;
    use brakwm_frontend::parser::Parser;
    use brakwm_ir_hir::lower::HirLower;
    use brakwm_ir_mir::lower::MirLower;

    fn lower_lir(src: &str) -> LirProgram {
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

    #[test]
    fn lowers_add_and_call() {
        let lir =
            lower_lir("fn add(a: i32, b: i32) -> i32 { a + b } fn main() -> i32 { add(10, 20) }");
        assert_eq!(lir.functions.len(), 2);
        assert_eq!(
            lir.functions[0].reg_types.len(),
            lir.functions[0].reg_count - 1
        );
        let total: usize = lir
            .functions
            .iter()
            .flat_map(|f| &f.blocks)
            .map(|b| b.insts.len())
            .sum();
        assert!(total > 0, "no LIR instructions were produced");
    }

    #[test]
    fn keeps_recursive_if_body() {
        let lir =
            lower_lir("fn fib(n: i32) -> i32 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } }");
        let fib = lir.functions.iter().find(|f| f.name == "fib").expect("fib");
        let total: usize = fib.blocks.iter().map(|b| b.insts.len()).sum();
        assert!(total > 0, "fib lowered to no instructions");
        assert!(
            fib.blocks.iter().any(|b| b
                .insts
                .iter()
                .any(|i| matches!(i.opcode, LirOpcode::Br | LirOpcode::Jmp | LirOpcode::Ret))),
            "fib has no control-flow instructions"
        );
    }

    #[test]
    fn interns_strings() {
        let lir = lower_lir(r#"fn s() -> string { "halo" }"#);
        assert_eq!(lir.string_table, vec!["halo".to_string()]);
    }
}
