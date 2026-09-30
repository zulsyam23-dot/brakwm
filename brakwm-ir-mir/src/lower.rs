use crate::mir::*;
use brakwm_core::{Result, Span};
use brakwm_ir_hir::hir::*;

/// Builds a CFG incrementally. All local ids are function-scoped.
pub struct MirBuilder {
    locals: Vec<MirLocal>,
    blocks: Vec<MirBlock>,
    cur: BlockId,
    next_block: usize,
    struct_tys: Vec<Option<String>>,
    enum_names: Vec<String>,
    /// Enum name -> variant -> discriminant, so a variant pattern can compare
    /// the scrutinee's tag against the pattern's tag.
    enum_tags: std::collections::HashMap<String, std::collections::HashMap<String, i64>>,
    /// Function name -> declared return type, so an aggregate returned by a
    /// call can be copied out of the callee's stack frame.
    fn_ret_tys: std::collections::HashMap<String, MirType>,
    /// Aggregate type name -> ordered field names, used for that copy.
    aggregate_fields: std::collections::HashMap<String, Vec<String>>,
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

/// Best-effort static type of an expression. Used to give statement-level
/// temporaries a real type so float and i64 operands survive lowering.
/// Comparison operators yield `Bool`; everything else falls back to `I32`.
fn expr_ty(e: &HirExpr) -> MirType {
    match e {
        HirExpr::Float(_, _) => MirType::F64,
        HirExpr::Bool(_, _) => MirType::Bool,
        HirExpr::String(_, _) => MirType::String,
        HirExpr::BinOp { op, lhs, .. } => match op {
            HirBinOp::Eq
            | HirBinOp::Ne
            | HirBinOp::Lt
            | HirBinOp::Le
            | HirBinOp::Gt
            | HirBinOp::Ge
            | HirBinOp::And
            | HirBinOp::Or => MirType::Bool,
            // Arithmetic keeps the operand type so float stays float.
            _ => {
                let t = expr_ty(lhs);
                if matches!(t, MirType::Void) {
                    MirType::I32
                } else {
                    t
                }
            }
        },
        HirExpr::UnOp { expr, .. } => {
            let t = expr_ty(expr);
            if matches!(t, MirType::Void) {
                MirType::I32
            } else {
                t
            }
        }
        HirExpr::If { .. } | HirExpr::Block(_) | HirExpr::Match { .. } => MirType::I32,
        _ => MirType::I32,
    }
}

impl MirBuilder {
    fn new() -> Self {
        let mut b = Self {
            locals: Vec::new(),
            blocks: Vec::new(),
            cur: 0,
            next_block: 0,
            struct_tys: Vec::new(),
            enum_names: Vec::new(),
            enum_tags: std::collections::HashMap::new(),
            fn_ret_tys: std::collections::HashMap::new(),
            aggregate_fields: std::collections::HashMap::new(),
        };
        b.alloc_block("entry".into());
        b
    }

    fn alloc_local(&mut self, name: &str, ty: MirType) -> LocalId {
        let id = self.locals.len();
        self.locals.push(MirLocal {
            name: name.to_string(),
            ty,
        });
        self.struct_tys.push(None);
        id
    }

    /// True when `name` is a declared enum, so `Enum.Variant` is an enum
    /// construction rather than a struct field access.
    fn is_enum(&self, name: &str) -> bool {
        self.enum_names.iter().any(|n| n == name)
    }

    /// Struct type name for a local, tracked so field access knows which
    /// layout to use. A bare `Named` local type is also accepted, which covers
    /// locals declared with an explicit annotation.
    fn struct_name_of(&self, id: LocalId) -> Option<String> {
        if let Some(Some(n)) = self.struct_tys.get(id) {
            return Some(n.clone());
        }
        match self.locals.get(id).map(|l| &l.ty) {
            Some(MirType::Named(n)) if n != "void" => Some(n.clone()),
            _ => None,
        }
    }

    /// Copy an aggregate returned by a call into fresh stack space.
    ///
    /// A struct pointer is really an address into the shadow stack, and a
    /// callee restores that stack on return. Without this copy two calls
    /// returning a struct of the same type would hand back the same address,
    /// so the second call silently overwrote the first caller's value. Every
    /// field is read out before the new allocation is made, because that
    /// allocation is what would clobber the original.
    fn copy_aggregate_result(&mut self, callee: &str, dest: LocalId) {
        let Some(MirType::Named(name)) = self.fn_ret_tys.get(callee).cloned() else {
            return;
        };
        let Some(fields) = self.aggregate_fields.get(&name).cloned() else {
            return;
        };
        if fields.is_empty() {
            return;
        }
        let src = dest;
        let mut reads = Vec::new();
        for field in &fields {
            let slot = self.alloc_local("agg_field", MirType::I32);
            self.emit(MirInst::Assign {
                dest: slot,
                value: MirValue::GetField {
                    object: src,
                    name: name.clone(),
                    field: field.clone(),
                },
                span: VOID_SPAN,
            });
            reads.push((field.clone(), slot));
        }
        self.emit(MirInst::Assign {
            dest,
            value: MirValue::StructInit {
                name,
                fields: reads,
            },
            span: VOID_SPAN,
        });
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
        // Propagate struct type info along plain register copies so that
        // `let p = P { .. }` keeps the layout for later `p.field` access.
        if let MirInst::Assign { dest, value, .. } = &inst {
            let src = match value {
                MirValue::Local(s) => self.struct_tys.get(*s).cloned().flatten(),
                MirValue::StructInit { name, .. } => Some(name.clone()),
                _ => None,
            };
            if let Some(name) = src {
                if let Some(slot) = self.struct_tys.get_mut(*dest) {
                    *slot = Some(name);
                }
            }
        }
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

    /// True when the current block already ended with `return`, so a trailing
    /// jump to a join block would be dead code.
    fn cur_returned(&self) -> bool {
        matches!(
            self.blocks[self.cur].terminator,
            MirTerminator::Return { .. }
        )
    }

    /// Jump to a join block unless the current block already returned. Without
    /// this, `if c { return 1; }` had its `return` overwritten by a jump and
    /// the early exit silently stopped working.
    fn jump_to(&mut self, target: BlockId) {
        if self.cur_returned() {
            return;
        }
        self.set_term(MirTerminator::Jump {
            target,
            span: VOID_SPAN,
        });
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
                // Operands must carry their real type; a Void operand becomes
                // an i32 local in LIR, which silently corrupts float math.
                let opnd_ty = expr_ty(lhs);
                let l = self.lower_expr(lhs, opnd_ty.clone())?;
                let r = self.lower_expr(rhs, opnd_ty)?;
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
                    value: MirValue::BinOp {
                        op: bop,
                        lhs: l,
                        rhs: r,
                    },
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
                    callee: callee_name.clone(),
                    args: arg_ids,
                    span: VOID_SPAN,
                });
                self.copy_aggregate_result(&callee_name, dest);
            }
            HirExpr::If {
                cond, then, else_, ..
            } => {
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
                // `Enum.Variant` builds an enum value. The object of a Field
                // node is the enum name itself, so match on `object` being an
                // Ident. The old code tested `object` for being a Field, which
                // never held and sent `Color.Red` down the struct path.
                if let HirExpr::Ident(enum_name, _) = &**object {
                    if self.is_enum(enum_name) {
                        // Materialise the discriminant into its own local:
                        // StructInit can only store from a register, so an
                        // inline tag value would be read back as register 0.
                        let tag = self
                            .enum_tags
                            .get(enum_name.as_str())
                            .and_then(|m| m.get(field.as_str()))
                            .copied()
                            .unwrap_or(0);
                        let tag_reg = self.alloc_local("enum_tag", MirType::I32);
                        self.emit(MirInst::Assign {
                            dest: tag_reg,
                            value: MirValue::Int(tag),
                            span: VOID_SPAN,
                        });
                        self.emit(MirInst::Assign {
                            dest,
                            value: MirValue::EnumInit {
                                enum_name: enum_name.clone(),
                                variant: field.clone(),
                                args: vec![tag_reg],
                            },
                            span: VOID_SPAN,
                        });
                        return Ok(());
                    }
                }
                if let HirExpr::Ident(name, _) = &**object {
                    let obj = self.find_local(name)?;
                    // Resolve the struct type, not the variable name: using
                    // the variable here made every field offset default to 0.
                    let sname = self.struct_name_of(obj).unwrap_or_default();
                    self.emit(MirInst::Assign {
                        dest,
                        value: MirValue::GetField {
                            object: obj,
                            name: sname,
                            field: field.clone(),
                        },
                        span: VOID_SPAN,
                    });
                } else {
                    let obj = self.lower_expr(object, MirType::Void)?;
                    let sname = self.struct_name_of(obj).unwrap_or_default();
                    self.emit(MirInst::Assign {
                        dest,
                        value: MirValue::GetField {
                            object: obj,
                            name: sname,
                            field: field.clone(),
                        },
                        span: VOID_SPAN,
                    });
                }
            }
            HirExpr::FieldAssign {
                object,
                field,
                value,
                ..
            } => {
                let obj = self.lower_expr(object, MirType::Void)?;
                let v = self.lower_expr(value, MirType::Void)?;
                let sname = self.struct_name_of(obj).unwrap_or_default();
                self.emit(MirInst::Assign {
                    dest,
                    value: MirValue::SetField {
                        object: obj,
                        name: sname,
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
                    value: MirValue::StructInit {
                        name: name.clone(),
                        fields: fids,
                    },
                    span: VOID_SPAN,
                });
                if let Some(slot) = self.struct_tys.get_mut(dest) {
                    *slot = Some(name.clone());
                }
            }
            HirExpr::EnumInit {
                enum_name,
                variant,
                args,
                ..
            } => {
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
                let scrutinee = self.lower_expr(expr, expr_ty(expr))?;
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
            self.emit(MirInst::Assign {
                dest: id,
                value: MirValue::Bool(false),
                span: VOID_SPAN,
            });
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
        self.set_term(MirTerminator::Jump {
            target: after,
            span: VOID_SPAN,
        });

        self.cur = else_block;
        self.lower_expr_to(else_, dest, dest_ty).unwrap();
        self.set_term(MirTerminator::Jump {
            target: after,
            span: VOID_SPAN,
        });

        self.cur = after;
    }

    fn lower_match(
        &mut self,
        scrutinee: LocalId,
        arms: &[(HirPattern, HirExpr)],
        after: BlockId,
        dest: LocalId,
    ) {
        // Each testable arm gets its own continuation block. Sharing a single
        // `default` block made every arm overwrite the previous arm's
        // terminator, silently dropping all but the first test.
        for (pat, body) in arms {
            match pat {
                HirPattern::Wildcard | HirPattern::Binding(_) => {
                    if let HirPattern::Binding(name) = pat {
                        let bind = self.alloc_local(name, MirType::I32);
                        self.emit(MirInst::Assign {
                            dest: bind,
                            value: MirValue::Local(scrutinee),
                            span: VOID_SPAN,
                        });
                    }
                    self.lower_expr_to(body, dest, expr_ty(body)).unwrap_or(());
                    self.set_term(MirTerminator::Jump {
                        target: after,
                        span: VOID_SPAN,
                    });
                    // Nothing below can be reached; give it a dead block so
                    // later arms never append to a terminated block.
                    self.cur = self.alloc_block("match_dead".into());
                    break;
                }
                HirPattern::Literal(lit) => {
                    let lty = match lit {
                        HirLiteral::Int(_) => MirType::I32,
                        HirLiteral::Bool(_) => MirType::Bool,
                        HirLiteral::Float(_) => MirType::F64,
                        HirLiteral::Str(_) => MirType::String,
                    };
                    let lit_id = self.alloc_local("pat_lit", lty);
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
                    let next = self.alloc_block("match_test".into());
                    let then = self.alloc_block("match_arm".into());
                    self.set_term(MirTerminator::Branch {
                        cond,
                        then,
                        else_: next,
                        span: VOID_SPAN,
                    });
                    self.cur = then;
                    self.lower_expr_to(body, dest, expr_ty(body)).unwrap_or(());
                    self.set_term(MirTerminator::Jump {
                        target: after,
                        span: VOID_SPAN,
                    });
                    self.cur = next;
                }
                HirPattern::Variant {
                    enum_name,
                    variant,
                    bindings,
                } => {
                    // Compare the scrutinee's discriminant against the
                    // pattern's. The old code branched on an `EnumInit` value
                    // itself, which is a struct pointer, not a boolean, so
                    // every variant pattern behaved like a truthy test and
                    // the first arm always won.
                    let tag = self
                        .enum_tags
                        .get(enum_name.as_str())
                        .and_then(|m| m.get(variant.as_str()))
                        .copied()
                        .unwrap_or(0);
                    let scrut_tag = self.alloc_local("scrut_tag", MirType::I32);
                    self.emit(MirInst::Assign {
                        dest: scrut_tag,
                        value: MirValue::GetField {
                            object: scrutinee,
                            name: format!("__enum_{}", enum_name),
                            field: "$tag".into(),
                        },
                        span: VOID_SPAN,
                    });
                    let pat_tag = self.alloc_local("pat_tag", MirType::I32);
                    self.emit(MirInst::Assign {
                        dest: pat_tag,
                        value: MirValue::Int(tag),
                        span: VOID_SPAN,
                    });
                    let tag_cond = self.alloc_local("tag_cond", MirType::Bool);
                    self.emit(MirInst::Assign {
                        dest: tag_cond,
                        value: MirValue::BinOp {
                            op: MirBinOp::Eq,
                            lhs: scrut_tag,
                            rhs: pat_tag,
                        },
                        span: VOID_SPAN,
                    });
                    let next = self.alloc_block("match_test".into());
                    let then = self.alloc_block("match_arm".into());
                    self.set_term(MirTerminator::Branch {
                        cond: tag_cond,
                        then,
                        else_: next,
                        span: VOID_SPAN,
                    });
                    self.cur = then;
                    for b in bindings {
                        self.alloc_local(b, MirType::I32);
                    }
                    self.lower_expr_to(body, dest, expr_ty(body)).unwrap_or(());
                    self.set_term(MirTerminator::Jump {
                        target: after,
                        span: VOID_SPAN,
                    });
                    self.cur = next;
                }
            }
        }
        // No arm matched: fall through the join block.
        self.set_term(MirTerminator::Jump {
            target: after,
            span: VOID_SPAN,
        });
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
            HirStmt::Let {
                name, ty, value, ..
            } => {
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
                // Infer the expression's real type: a `Void` temp would be
                // registered as i32 in LIR, which corrupts float and i64
                // operands before they reach codegen.
                let ty = expr_ty(e);
                let tmp = self.alloc_local("stmt_tmp", ty.clone());
                self.lower_expr_to(e, tmp, ty)?;
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
            HirStmt::If {
                cond, then, else_, ..
            } => {
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
                self.jump_to(after);
                self.cur = else_block;
                if let Some(e) = else_ {
                    self.lower_block(e);
                }
                self.jump_to(after);
                self.cur = after;
            }
            HirStmt::While { cond, body, .. } => {
                let header = self.alloc_block("while_header".into());
                let body_block = self.alloc_block("while_body".into());
                let exit = self.alloc_block("while_exit".into());
                self.set_term(MirTerminator::Jump {
                    target: header,
                    span: VOID_SPAN,
                });
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
                self.set_term(MirTerminator::Jump {
                    target: header,
                    span: VOID_SPAN,
                });
                self.cur = exit;
            }
            HirStmt::Loop { body, .. } => {
                let header = self.alloc_block("loop_header".into());
                self.set_term(MirTerminator::Jump {
                    target: header,
                    span: VOID_SPAN,
                });
                self.cur = header;
                self.lower_block(body);
                self.set_term(MirTerminator::Jump {
                    target: header,
                    span: VOID_SPAN,
                });
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

pub struct MirLower {
    /// Enum names declared in the program, so `Enum.Variant` can be told
    /// apart from a struct field access during lowering.
    enum_names: Vec<String>,
    /// Enum name -> variant -> discriminant, used to match variant patterns.
    enum_tags: std::collections::HashMap<String, std::collections::HashMap<String, i64>>,
    /// Function name -> declared return type, so an aggregate return can be
    /// copied out of the callee's stack frame.
    fn_ret_tys: std::collections::HashMap<String, MirType>,
    /// Aggregate type name -> ordered field names. Enums are stored as the
    /// synthetic `__enum_<Name>` with a leading `$tag` discriminant.
    aggregate_fields: std::collections::HashMap<String, Vec<String>>,
}

impl MirLower {
    pub fn new() -> Self {
        Self {
            enum_names: Vec::new(),
            enum_tags: std::collections::HashMap::new(),
            fn_ret_tys: std::collections::HashMap::new(),
            aggregate_fields: std::collections::HashMap::new(),
        }
    }

    pub fn lower(&mut self, program: HirProgram) -> Result<MirProgram> {
        let mut functions = Vec::new();
        let mut extern_functions = Vec::new();
        let mut structs = Vec::new();
        let mut enums = Vec::new();

        for item in &program.items {
            if let HirItem::Enum(e) = item {
                self.enum_names.push(e.name.clone());
                let tags = self.enum_tags.entry(e.name.clone()).or_default();
                for (i, v) in e.variants.iter().enumerate() {
                    tags.insert(v.name.clone(), i as i64);
                }
                // Every variant is stored as $tag plus its payload fields, so a
                // copy has to move the discriminant and the widest payload.
                let arity = e
                    .variants
                    .iter()
                    .map(|v| v.fields.as_ref().map(|f| f.len()).unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                let mut fields = vec!["$tag".to_string()];
                fields.extend((0..arity).map(|i| format!("${i}")));
                self.aggregate_fields
                    .insert(format!("__enum_{}", e.name), fields.clone());
                self.aggregate_fields.insert(e.name.clone(), fields);
            }
        }

        for item in &program.items {
            match item {
                HirItem::Function(f) => {
                    self.fn_ret_tys.insert(f.name.clone(), mir_ty(&f.ret_ty));
                }
                HirItem::ExternFunction(e) => {
                    self.fn_ret_tys.insert(e.name.clone(), mir_ty(&e.ret_ty));
                }
                HirItem::Struct(s) => {
                    self.aggregate_fields.insert(
                        s.name.clone(),
                        s.fields.iter().map(|f| f.name.clone()).collect(),
                    );
                }
                _ => {}
            }
        }

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
                                fields: v
                                    .fields
                                    .as_ref()
                                    .map(|tys| tys.iter().map(mir_ty).collect()),
                                span: v.span,
                            })
                            .collect(),
                        span: e.span,
                    });
                }
                _ => {}
            }
        }

        Ok(MirProgram {
            functions,
            extern_functions,
            structs,
            enums,
        })
    }

    fn lower_function(&mut self, f: &HirFunction) -> Result<MirFunction> {
        let mut b = MirBuilder::new();
        b.enum_names = self.enum_names.clone();
        b.enum_tags = self.enum_tags.clone();
        b.fn_ret_tys = self.fn_ret_tys.clone();
        b.aggregate_fields = self.aggregate_fields.clone();
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
        let params = f
            .params
            .iter()
            .map(|p| (p.name.clone(), mir_ty(&p.ty)))
            .collect();
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
        let m =
            lower_mir("fn add(a: i32, b: i32) -> i32 { a + b } fn main() -> i32 { add(10, 20) }");
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
        let total_insts: usize = m
            .functions
            .iter()
            .flat_map(|f| &f.blocks)
            .map(|b| b.insts.len())
            .sum();
        assert!(total_insts > 0, "no instructions were lowered");
    }

    /// Every arm of a multi-arm match must be reachable through its own test
    /// block. Sharing one continuation block let later arms overwrite earlier
    /// terminators, so only the first test ever ran.
    #[test]
    fn lowers_multi_arm_match() {
        let m = lower_mir("fn g(n: i32) -> i32 { match n { 0 => 10, 1 => 20, 2 => 30, _ => 99 } }");
        let f = &m.functions[0];
        for block in &f.blocks {
            assert!(
                !matches!(block.terminator, MirTerminator::Unreachable),
                "block {} of {} has no terminator",
                block.id,
                f.name
            );
        }
        let tests = f
            .blocks
            .iter()
            .filter(|b| matches!(b.terminator, MirTerminator::Branch { .. }))
            .count();
        assert_eq!(tests, 3, "expected one branch per literal arm, got {tests}");
    }

    #[test]
    fn lowers_early_return_without_extra_jump() {
        let m = lower_mir("fn f(n: i32) -> i32 { if n > 0 { return 7; } 0 }");
        let f = &m.functions[0];
        let then_block = f
            .blocks
            .iter()
            .find(|b| b.name == "if_then")
            .expect("if_then block");
        assert!(
            matches!(then_block.terminator, MirTerminator::Return { .. }),
            "early return must survive, got {:?}",
            then_block.terminator
        );
    }

    #[test]
    fn struct_field_access_carries_type_name() {
        let m = lower_mir(
            "struct P { x: i32, y: i32 } fn main() -> i32 { let p = P { x: 1, y: 2 }; p.x + p.y }",
        );
        let f = m.functions.iter().find(|f| f.name == "main").expect("main");
        let has_named_field = f.blocks.iter().any(|b| {
            b.insts.iter().any(|i| {
                matches!(
                    i,
                    MirInst::Assign {
                        value: MirValue::GetField { name, .. },
                        ..
                    } if name == "P"
                )
            })
        });
        assert!(
            has_named_field,
            "GetField must record the struct type name, not the variable name"
        );
    }

    /// A callee restores the shadow stack on return, so two calls returning the
    /// same struct type used to hand back the same address. The second call
    /// then overwrote the first result.
    #[test]
    fn aggregate_call_result_is_copied_into_the_caller() {
        let m = lower_mir(
            "struct P { x: i32 }\n\
             fn make(v: i32) -> P { P { x: v } }\n\
             fn main() -> i32 { let a = make(1); let b = make(2); a.x + b.x }",
        );
        let f = m.functions.iter().find(|f| f.name == "main").expect("main");
        let copies_after_call = f.blocks.iter().any(|b| {
            let mut seen_call = false;
            b.insts.iter().any(|i| match i {
                MirInst::Call { callee, .. } if callee == "make" => {
                    seen_call = true;
                    false
                }
                MirInst::Assign {
                    value: MirValue::StructInit { name, .. },
                    ..
                } => seen_call && name == "P",
                _ => false,
            })
        });
        assert!(
            copies_after_call,
            "an aggregate returned by a call must be reallocated in the caller"
        );
    }

    /// A variant pattern has to compare the scrutinee's discriminant against the
    /// pattern's own tag, which means reading `$tag` off the synthetic enum
    /// layout rather than the first payload slot.
    #[test]
    fn enum_variant_pattern_reads_the_tag_field() {
        let m = lower_mir(
            "enum Color { Red, Green, Blue }\n\
             fn tag(c: Color) -> i32 { match c { Color.Red => 1, Color.Green => 2, _ => 3 } }\n\
             fn main() -> i32 { tag(Color.Green) }",
        );
        let f = m.functions.iter().find(|f| f.name == "tag").expect("tag");
        let reads_tag = f.blocks.iter().any(|b| {
            b.insts.iter().any(|i| {
                matches!(
                    i,
                    MirInst::Assign {
                        value: MirValue::GetField { field, .. },
                        ..
                    } if field == "$tag"
                )
            })
        });
        assert!(
            reads_tag,
            "variant patterns must load $tag from the enum layout"
        );
    }

    /// The discriminant has to reach StructInit through a register: passing the
    /// literal directly made every enum read back as register 0.
    #[test]
    fn enum_init_materialises_the_discriminant() {
        let m = lower_mir(
            "enum Color { Red, Green }\n\
             fn main() -> i32 { let c = Color.Green; 0 }",
        );
        let f = m.functions.iter().find(|f| f.name == "main").expect("main");
        let materialized = f.blocks.iter().any(|b| {
            let mut seen_tag_local = false;
            b.insts.iter().any(|i| match i {
                MirInst::Assign {
                    value: MirValue::Int(_),
                    ..
                } => {
                    seen_tag_local = true;
                    false
                }
                MirInst::Assign {
                    value: MirValue::EnumInit { args, .. },
                    ..
                } => seen_tag_local && !args.is_empty(),
                _ => false,
            })
        });
        assert!(
            materialized,
            "the variant discriminant must be stored in a local before StructInit"
        );
    }
}
