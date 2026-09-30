use brakwm_core::{Result, Span};
use brakwm_ir_hir::hir::*;
use crate::mir::*;

/// Builds a CFG incrementally. All local ids are function-scoped.
pub struct MirBuilder {
    locals: Vec<MirLocal>,
    blocks: Vec<MirBlock>,
    cur: BlockId,
    next_block: usize,
}

fn mir_ty(t: &HirType) -> MirType {
    match t {
        HirType::I32 => MirType::I32,
        HirType::I64 => MirType::I64,
        HirType::F32 => MirType::F32,
        HirType::F64 => MirType::F64,
        HirType::Bool => MirType::Bool,
        HirType::String => MirType::String,
        HirType::Void => MirType::Void,
        HirType::Named(s) => MirType::Named(s.clone()),
        _ => MirType::I32,
    }
}

const VOID_SPAN: Span = Span {
    start: brakwm_core::SourceLoc::new(0, 0, 0),
    end: brakwm_core::SourceLoc::new(0, 0, 0),
};

impl MirBuilder {
    fn new() -> Self {
        let mut b = Self {
            locals: Vec::new(),
            blocks: Vec::new(),
            cur: 0,
            next_block: 0,
        };
        b.alloc_block("entry".into());
        b
    }

    fn alloc_local(&mut self, name: &str, ty: MirType) -> LocalId {
        let id = self.locals.len();
        self.locals.push(MirLocal { name: name.to_string(), ty });
        id
    }

    fn alloc_block(&mut self, name: String) -> BlockId {
        let id = self.next_block;
        self.next_block += 1;
        self.blocks.push(MirBlock {
            id,
            name,
            insts: Vec::new(),
            terminator: MirTerminator::Unreachable,
            span: VOID_SPAN,
        });
        id
    }

    fn emit(&mut self, inst: MirInst) {
        self.blocks[self.cur].insts.push(inst);
    }

    fn set_term(&mut self, term: MirTerminator) {
        let term_span = match &term {
            MirTerminator::Return { span, .. } => *span,
            MirTerminator::Jump { span, .. } => *span,
            MirTerminator::Branch { span, .. } => *span,
            MirTerminator::Unreachable => VOID_SPAN,
        };
        self.blocks[self.cur].terminator = term;
        self.blocks[self.cur].span = term_span;
    }

    // -------- expression lowering --------

    fn lower_expr(&mut self, expr: &HirExpr, dest_ty: MirType) -> Result<LocalId> {
        let dest = self.alloc_local("tmp", dest_ty.clone());
        self.lower_expr_to(expr, dest, dest_ty)?;
        Ok(dest)
    }

    fn lower_expr_to(&mut self, expr: &HirExpr, dest: LocalId, dest_ty: MirType) -> Result<()> {
        match expr {
            HirExpr::Int(v, _) => {
                self.emit(MirInst::Assign {
                    dest,
                    value: MirValue::Int(*v),
                    span: VOID_SPAN,
                });
            }
            HirExpr::Float(v, _) => {
                self.emit(MirInst::Assign {
                    dest,
                    value: MirValue::Float(*v),
                    span: VOID_SPAN,
                });
            }
            HirExpr::Bool(v, _) => {
                self.emit(MirInst::Assign {
                    dest,
                    value: MirValue::Bool(*v),
                    span: VOID_SPAN,
                });
            }
            HirExpr::String(v, _) => {
                self.emit(MirInst::Assign {
                    dest,
                    value: MirValue::String(v.clone()),
                    span: VOID_SPAN,
                });
            }
            HirExpr::Ident(name, _) => {
                let id = self.find_local(name)?;
                self.emit(MirInst::Assign {
                    dest,
                    value: MirValue::Local(id),
                    span: VOID_SPAN,
                });
            }
            HirExpr::Assign(name, value, _) => {
                let target = self.find_local(name)?;
                let v = self.lower_expr(value, MirType::Void)?;
                self.emit(MirInst::Assign {
                    dest: target,
                    value: MirValue::Local(v),
                    span: VOID_SPAN,
                });
                self.emit(MirInst::Assign {
                    dest,
                    value: MirValue::Local(target),
                    span: VOID_SPAN,
                });
            }
            HirExpr::BinOp { op, lhs, rhs, .. } => {
                let l = self.lower_expr(lhs, MirType::Void)?;
                let r = self.lower_expr(rhs, MirType::Void)?;
                let bop = match op {
                    HirBinOp::Add => MirBinOp::Add,
                    HirBinOp::Sub => MirBinOp::Sub,
                    HirBinOp::Mul => MirBinOp::Mul,
                    HirBinOp::Div => MirBinOp::Div,
                    HirBinOp::Mod => MirBinOp::Mod,
                    HirBinOp::Eq => MirBinOp::Eq,
                    HirBinOp::Ne => MirBinOp::Ne,
                    HirBinOp::Lt => MirBinOp::Lt,
                    HirBinOp::Le => MirBinOp::Le,
                    HirBinOp::Gt => MirBinOp::Gt,
                    HirBinOp::Ge => MirBinOp::Ge,
                    HirBinOp::And => MirBinOp::And,
                    HirBinOp::Or => MirBinOp::Or,
                    HirBinOp::BitAnd => MirBinOp::BitAnd,
                    HirBinOp::BitOr => MirBinOp::BitOr,
                    HirBinOp::BitXor => MirBinOp::BitXor,
                    HirBinOp::Shl => MirBinOp::Shl,
                    HirBinOp::Shr => MirBinOp::Shr,
                    HirBinOp::Range => MirBinOp::Add, // not used in MIR
                };
                self.emit(MirInst::Assign {
                    dest,
                    value: MirValue::BinOp { op: bop, lhs: l, rhs: r },
                    span: VOID_SPAN,
                });
            }
            HirExpr::UnOp { op, expr, .. } => {
                let e = self.lower_expr(expr, MirType::Void)?;
                let uop = match op {
                    HirUnOp::Neg => MirUnOp::Neg,
                    HirUnOp::Not => MirUnOp::Not,
                    HirUnOp::BitNot => MirUnOp::BitNot,
                };
                self.emit(MirInst::Assign {
                    dest,
                    value: MirValue::UnOp { op: uop, expr: e },
                    span: VOID_SPAN,
                });
            }
            HirExpr::Call { callee, args, .. } => {
                let callee_name = match &**callee {
                    HirExpr::Ident(n, _) => n.clone(),
                    _ => return Err(format!("only direct calls supported").into()),
                };
                let mut arg_ids = Vec::new();
                for a in args {
                    let id = self.lower_expr(a, MirType::Void)?;
                    arg_ids.push(id);
                }
                self.emit(MirInst::Call {
                    dest: Some(dest),
                    callee: callee_name,
                    args: arg_ids,
                    span: VOID_SPAN,
                });
            }
            HirExpr::If { cond, then, else_, .. } => {
                let after = self.alloc_block("if_after".into());
                self.lower_if(cond, then, else_, after, dest, dest_ty);
            }
            HirExpr::Block(b) => {
                if b.tail.is_some() {
                    self.lower_block_to(b, dest, dest_ty);
                } else {
                    self.lower_block(b);
                }
            }
            HirExpr::Field { object, field, .. } => {
                // Enum construction: TypeName.Variant or struct field access
                if let HirExpr::Field { object, field, .. } = &**object {
                    // `Enum.Variant` — treat as EnumInit with no args
                    if let HirExpr::Ident(enum_name, _) = &**object {
                        self.emit(MirInst::Assign {
                            dest,
                            value: MirValue::EnumInit {
                                enum_name: enum_name.clone(),
                                variant: field.clone(),
                                args: Vec::new(),
                            },
                            span: VOID_SPAN,
                        });
                        return Ok(());
                    }
                }
                if let HirExpr::Ident(name, _) = &**object {
                    let obj = self.find_local(name)?;
                    self.emit(MirInst::Assign {
                        dest,
                        value: MirValue::GetField {
                            object: obj,
                            name: name.clone(),
                            field: field.clone(),
                        },
                        span: VOID_SPAN,
                    });
                } else {
                    let obj = self.lower_expr(object, MirType::Void)?;
                    self.emit(MirInst::Assign {
                        dest,
                        value: MirValue::GetField {
                            object: obj,
                            name: String::new(),
                            field: field.clone(),
                        },
                        span: VOID_SPAN,
                    });
                }
            }
            HirExpr::FieldAssign { object, field, value, .. } => {
                let obj = self.lower_expr(object, MirType::Void)?;
                let v = self.lower_expr(value, MirType::Void)?;
                self.emit(MirInst::Assign {
                    dest,
                    value: MirValue::SetField {
                        object: obj,
                        name: String::new(),
                        field: field.clone(),
                        value: v,
                    },
                    span: VOID_SPAN,
                });
            }
            HirExpr::StructInit { name, fields, .. } => {
                let mut fids = Vec::new();
                for (fname, fexpr) in fields {
                    let id = self.lower_expr(fexpr, MirType::Void)?;
                    fids.push((fname.clone(), id));
                }
                self.emit(MirInst::Assign {
                    dest,
                    value: MirValue::StructInit { name: name.clone(), fields: fids },
                    span: VOID_SPAN,
                });
            }
            HirExpr::EnumInit { enum_name, variant, args, .. } => {
                let mut arg_ids = Vec::new();
                for a in args {
                    let id = self.lower_expr(a, MirType::Void)?;
                    arg_ids.push(id);
                }
                self.emit(MirInst::Assign {
                    dest,
                    value: MirValue::EnumInit {
                        enum_name: enum_name.clone(),
                        variant: variant.clone(),
                        args: arg_ids,
                    },
                    span: VOID_SPAN,
                });
            }
            HirExpr::Match { expr, arms, .. } => {
                let scrutinee = self.lower_expr(expr, MirType::Void)?;
                let after = self.alloc_block("match_after".into());
                self.lower_match(scrutinee, arms, after, dest);
            }
        }
        Ok(())
    }

    fn lower_if(
        &mut self,
        cond: &HirExpr,
        then: &HirExpr,
        else_: &HirExpr,
        after: BlockId,
        dest: LocalId,
        dest_ty: MirType,
    ) {
        let c = self.lower_expr(cond, MirType::Bool).unwrap_or_else(|_| {
            let id = self.alloc_local("tmp", MirType::Bool);
            self.emit(MirInst::Assign { dest: id, value: MirValue::Bool(false), span: VOID_SPAN });
            id
        });
        let then_block = self.alloc_block("if_then".into());
        let else_block = self.alloc_block("if_else".into());
        self.set_term(MirTerminator::Branch {
            cond: c,
            then: then_block,
            else_: else_block,
            span: VOID_SPAN,
        });

        self.cur = then_block;
        self.lower_expr_to(then, dest, dest_ty.clone()).unwrap();
        self.set_term(MirTerminator::Jump { target: after, span: VOID_SPAN });

        self.cur = else_block;
        self.lower_expr_to(else_, dest, dest_ty).unwrap();
        self.set_term(MirTerminator::Jump { target: after, span: VOID_SPAN });

        self.cur = after;
    }

    fn lower_match(&mut self, scrutinee: LocalId, arms: &[(HirPattern, HirExpr)], after: BlockId, dest: LocalId) {
        let default_block = self.alloc_block("match_default".into());
        for (pat, body) in arms {
            match pat {
                HirPattern::Wildcard => {
                    self.cur = default_block;
                    self.lower_expr_to(body, dest, MirType::Void).unwrap_or(());
                    self.set_term(MirTerminator::Jump { target: after, span: VOID_SPAN });
                    self.cur = default_block;
                    return;
                }
                HirPattern::Binding(name) => {
                    let bind = self.alloc_local(name, MirType::I32);
                    self.emit(MirInst::Assign {
                        dest: bind,
                        value: MirValue::Local(scrutinee),
                        span: VOID_SPAN,
                    });
                    self.cur = default_block;
                    self.lower_expr_to(body, dest, MirType::Void).unwrap_or(());
                    self.set_term(MirTerminator::Jump { target: after, span: VOID_SPAN });
                    self.cur = default_block;
                    return;
                }
                HirPattern::Literal(lit) => {
                    let lit_id = self.alloc_local("pat_lit", MirType::I32);
                    self.emit(MirInst::Assign {
                        dest: lit_id,
                        value: match lit {
                            HirLiteral::Int(i) => MirValue::Int(*i),
                            HirLiteral::Bool(b) => MirValue::Bool(*b),
                            HirLiteral::Float(f) => MirValue::Float(*f),
                            HirLiteral::Str(s) => MirValue::String(s.clone()),
                        },
                        span: VOID_SPAN,
                    });
                    let cond = self.alloc_local("pat_cond", MirType::Bool);
                    self.emit(MirInst::Assign {
                        dest: cond,
                        value: MirValue::BinOp {
                            op: MirBinOp::Eq,
                            lhs: scrutinee,
                            rhs: lit_id,
                        },
                        span: VOID_SPAN,
                    });
                    let then = self.alloc_block("match_arm".into());
                    self.set_term(MirTerminator::Branch {
                        cond,
                        then,
                        else_: default_block,
                        span: VOID_SPAN,
                    });
                    self.cur = then;
                    self.lower_expr_to(body, dest, MirType::Void).unwrap_or(());
                    self.set_term(MirTerminator::Jump { target: after, span: VOID_SPAN });
                    self.cur = default_block;
                }
                HirPattern::Variant { enum_name, variant, bindings } => {
                    let tag_cond = self.alloc_local("tag_cond", MirType::Bool);
                    self.emit(MirInst::Assign {
                        dest: tag_cond,
                        value: MirValue::EnumInit {
                            enum_name: enum_name.clone(),
                            variant: variant.clone(),
                            args: Vec::new(),
                        },
                        span: VOID_SPAN,
                    });
                    let then = self.alloc_block("match_arm".into());
                    self.set_term(MirTerminator::Branch {
                        cond: tag_cond,
                        then,
                        else_: default_block,
                        span: VOID_SPAN,
                    });
                    self.cur = then;
                    // bind payload args (fieldless for now)
                    for b in bindings {
                        self.alloc_local(b, MirType::I32);
                    }
                    self.lower_expr_to(body, dest, MirType::Void).unwrap_or(());
                    self.set_term(MirTerminator::Jump { target: after, span: VOID_SPAN });
                    self.cur = default_block;
                }
            }
        }
        self.set_term(MirTerminator::Jump { target: after, span: VOID_SPAN });
        self.cur = after;
    }

    // -------- statements --------

    fn lower_block(&mut self, block: &HirBlock) {
        for stmt in &block.stmts {
            self.lower_stmt(stmt).unwrap_or(());
        }
    }

    /// Lower a block and, if it has a tail expression, assign it to `dest`.
    fn lower_block_to(&mut self, block: &HirBlock, dest: LocalId, ty: MirType) {
        self.lower_block(block);
        if let Some(tail) = &block.tail {
            let src = self
                .lower_expr(tail, ty.clone())
                .expect("tail expression must lower");
            self.emit(MirInst::Assign {
                dest,
                value: MirValue::Local(src),
                span: VOID_SPAN,
            });
        }
    }

    fn lower_stmt(&mut self, stmt: &HirStmt) -> Result<()> {
        match stmt {
            HirStmt::Let { name, ty, value, .. } => {
                let id = self.alloc_local(name, mir_ty(ty));
                if let Some(v) = value {
                    let src = self.lower_expr(v, mir_ty(ty))?;
                    self.emit(MirInst::Assign {
                        dest: id,
                        value: MirValue::Local(src),
                        span: VOID_SPAN,
                    });
                }
            }
            HirStmt::Expr(e, _) => {
                let tmp = self.alloc_local("stmt_tmp", MirType::Void);
                self.lower_expr_to(e, tmp, MirType::Void)?;
            }
            HirStmt::Return(v, span) => {
                let value = match v {
                    Some(e) => {
                        let id = self.lower_expr(e, MirType::Void)?;
                        Some(id)
                    }
                    None => None,
                };
                self.set_term(MirTerminator::Return { value, span: *span });
            }
            HirStmt::If { cond, then, else_, .. } => {
                let c = self.lower_expr(cond, MirType::Bool)?;
                let then_block = self.alloc_block("if_then".into());
                let else_block = self.alloc_block("if_else".into());
                self.set_term(MirTerminator::Branch {
                    cond: c,
                    then: then_block,
                    else_: else_block,
                    span: VOID_SPAN,
                });
                self.cur = then_block;
                self.lower_block(then);
                let after = self.alloc_block("if_after".into());
                self.set_term(MirTerminator::Jump { target: after, span: VOID_SPAN });
                self.cur = else_block;
                if let Some(e) = else_ {
                    self.lower_block(e);
                }
                self.set_term(MirTerminator::Jump { target: after, span: VOID_SPAN });
                self.cur = after;
            }
            HirStmt::While { cond, body, .. } => {
                let header = self.alloc_block("while_header".into());
                let body_block = self.alloc_block("while_body".into());
                let exit = self.alloc_block("while_exit".into());
                self.set_term(MirTerminator::Jump { target: header, span: VOID_SPAN });
                self.cur = header;
                let c = self.lower_expr(cond, MirType::Bool)?;
                self.set_term(MirTerminator::Branch {
                    cond: c,
                    then: body_block,
                    else_: exit,
                    span: VOID_SPAN,
                });
                self.cur = body_block;
                self.lower_block(body);
                self.set_term(MirTerminator::Jump { target: header, span: VOID_SPAN });
                self.cur = exit;
            }
            HirStmt::Loop { body, .. } => {
                let header = self.alloc_block("loop_header".into());
                self.set_term(MirTerminator::Jump { target: header, span: VOID_SPAN });
                self.cur = header;
                self.lower_block(body);
                self.set_term(MirTerminator::Jump { target: header, span: VOID_SPAN });
            }
            HirStmt::Break(_) | HirStmt::Continue(_) => {
                // handled by enclosing loop context; treated as no-op fallthrough
            }
        }
        Ok(())
    }

    fn find_local(&self, name: &str) -> Result<LocalId> {
        self.locals
            .iter()
            .rposition(|l| l.name == name)
            .ok_or_else(|| format!("undefined variable '{name}'").into())
    }

    fn finish(self, name: String, params: Vec<(String, MirType)>, ret_ty: MirType) -> MirFunction {
        // `lower_function` allocates params as the first locals, so the local
        // list already has params at ids 0..n. Reordering or deduplicating here
        // would invalidate every LocalId used in the emitted instructions.
        let param_ids: Vec<usize> = (0..params.len()).collect();

        MirFunction {
            name,
            params: param_ids,
            ret_ty,
            blocks: self.blocks,
            locals: self.locals,
            span: VOID_SPAN,
        }
    }
}

pub struct MirLower;

impl MirLower {
    pub fn new() -> Self {
        Self
    }

    pub fn lower(&mut self, program: HirProgram) -> Result<MirProgram> {
        let mut functions = Vec::new();
        let mut extern_functions = Vec::new();
        let mut structs = Vec::new();
        let mut enums = Vec::new();

        for item in &program.items {
            match item {
                HirItem::Function(f) => {
                    functions.push(self.lower_function(f)?);
                }
                HirItem::ExternFunction(e) => {
                    extern_functions.push(MirExternFunction {
                        name: e.name.clone(),
                        params: e.params.iter().map(|p| mir_ty(&p.ty)).collect(),
                        ret_ty: mir_ty(&e.ret_ty),
                        abi: e.abi.clone(),
                        span: e.span,
                    });
                }
                HirItem::Struct(s) => {
                    structs.push(MirStruct {
                        name: s.name.clone(),
                        fields: s
                            .fields
                            .iter()
                            .map(|f| MirField {
                                name: f.name.clone(),
                                ty: mir_ty(&f.ty),
                                span: f.span,
                            })
                            .collect(),
                        span: s.span,
                    });
                }
                HirItem::Enum(e) => {
                    enums.push(MirEnum {
                        name: e.name.clone(),
                        variants: e
                            .variants
                            .iter()
                            .map(|v| MirVariant {
                                name: v.name.clone(),
                                fields: v.fields.as_ref().map(|tys| tys.iter().map(mir_ty).collect()),
                                span: v.span,
                            })
                            .collect(),
                        span: e.span,
                    });
                }
                _ => {}
            }
        }

        Ok(MirProgram { functions, extern_functions, structs, enums })
    }

    fn lower_function(&mut self, f: &HirFunction) -> Result<MirFunction> {
        let mut b = MirBuilder::new();
        // Allocate params as the first locals.
        for p in &f.params {
            b.alloc_local(&p.name, mir_ty(&p.ty));
        }
        let ret_ty = mir_ty(&f.ret_ty);
        let value = if matches!(ret_ty, MirType::Void) {
            b.lower_block(&f.body);
            None
        } else {
            let tmp = b.alloc_local("tail_tmp", ret_ty.clone());
            b.lower_block_to(&f.body, tmp, ret_ty.clone());
            Some(tmp)
        };
        b.set_term(MirTerminator::Return {
            value,
            span: VOID_SPAN,
        });
        let params = f.params.iter().map(|p| (p.name.clone(), mir_ty(&p.ty))).collect();
        Ok(b.finish(f.name.clone(), params, ret_ty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brakwm_core::SourceMap;
    use brakwm_frontend::parser::Parser;
    use brakwm_ir_hir::lower::HirLower;

    fn lower_mir(src: &str) -> MirProgram {
        let sm = SourceMap::new("t.brk", src);
        let mut p = Parser::new();
        let ast = p.parse_source(&sm).unwrap();
        let hir = HirLower::new().lower(ast).unwrap();
        let mut ml = MirLower::new();
        ml.lower(hir).unwrap()
    }

    #[test]
    fn lowers_call() {
        let m = lower_mir("fn add(a: i32, b: i32) -> i32 { a + b } fn main() -> i32 { add(10, 20) }");
        assert_eq!(m.functions.len(), 2);
    }

    #[test]
    fn lowers_while() {
        let m = lower_mir("fn f() { let i: i32 = 0; while i < 5 { i = i + 1; } }");
        assert_eq!(m.functions.len(), 1);
        assert!(m.functions[0].blocks.len() >= 4);
    }

    #[test]
    fn lowers_fib_recursion() {
        let m = lower_mir("fn fib(n: i32) -> i32 { if n <= 1 { n } else { fib(n - 1) + fib(n - 2) } } fn main() -> i32 { fib(10) }");
        assert_eq!(m.functions.len(), 2);
        // Every block must carry a real terminator, not the default Unreachable.
        for func in &m.functions {
            for block in &func.blocks {
                assert!(
                    !matches!(block.terminator, MirTerminator::Unreachable),
                    "block {} of {} has no terminator",
                    block.id,
                    func.name
                );
            }
        }
        let total_insts: usize = m.functions.iter().flat_map(|f| &f.blocks).map(|b| b.insts.len()).sum();
        assert!(total_insts > 0, "no instructions were lowered");
    }
}