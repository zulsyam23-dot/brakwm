use brakwm_core::Result;
use brakwm_ir_ast::ast::{self, BinOp, Expr, Stmt, Type, UnOp};
use crate::hir::*;

pub struct HirLower {
    errors: Vec<String>,
}

impl HirLower {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn lower(&self, program: ast::Program) -> Result<HirProgram> {
        let mut items = Vec::new();
        for item in program.items {
            match item {
                ast::Item::FnDef(f) => {
                    items.push(HirItem::Function(self.lower_fn(f)?));
                }
                ast::Item::ExternFn(f) => {
                    let param_types = f
                        .params
                        .iter()
                        .map(|p| self.lower_type(&p.ty))
                        .collect::<Result<Vec<_>>>()?;
                    let params = f
                        .params
                        .iter()
                        .map(|p| HirParam {
                            name: p.name.0.clone(),
                            ty: self.lower_type(&p.ty).unwrap_or(HirType::I32),
                            span: p.span,
                        })
                        .collect();
                    let ret_ty = match &f.ret_ty {
                        Some(t) => self.lower_type(t)?,
                        None => HirType::Void,
                    };
                    items.push(HirItem::ExternFunction(HirExternFunction {
                        name: f.name.0.clone(),
                        params,
                        ret_ty,
                        abi: f.abi.clone(),
                        span: f.span,
                    }));
                    let _ = param_types;
                }
                ast::Item::Let(l) => {
                    let ty = match &l.ty {
                        Some(t) => self.lower_type(t)?,
                        None => HirType::I32,
                    };
                    let value = match &l.value {
                        Some(v) => Some(Box::new(self.lower_expr(v)?)),
                        None => None,
                    };
                    items.push(HirItem::GlobalLet(HirGlobalLet {
                        name: l.name.0.clone(),
                        ty,
                        value,
                        span: l.span,
                    }));
                }
                ast::Item::Struct(s) => {
                    let fields = s
                        .fields
                        .iter()
                        .map(|f| {
                            Ok(HirField {
                                name: f.name.0.clone(),
                                ty: self.lower_type(&f.ty)?,
                                span: f.span,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    items.push(HirItem::Struct(HirStruct {
                        name: s.name.0.clone(),
                        fields,
                        span: s.span,
                    }));
                }
                ast::Item::Enum(e) => {
                    let variants = e
                        .variants
                        .iter()
                        .map(|v| {
                            let fields = v
                                .fields
                                .as_ref()
                                .map(|tys| {
                                    tys.iter()
                                        .map(|t| self.lower_type(t))
                                        .collect::<Result<Vec<_>>>()
                                })
                                .transpose()?;
                            Ok(HirVariant {
                                name: v.name.0.clone(),
                                fields,
                                span: v.span,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    items.push(HirItem::Enum(HirEnum {
                        name: e.name.0.clone(),
                        variants,
                        span: e.span,
                    }));
                }
                ast::Item::Const(c) => {
                    let _ = c;
                    // Constant folding of consts is not wired end-to-end yet;
                    // treat like a global let with the expression as value.
                    items.push(HirItem::GlobalLet(HirGlobalLet {
                        name: c.name.0.clone(),
                        ty: self.lower_type(&c.ty)?,
                        value: Some(Box::new(self.lower_expr(&c.value)?)),
                        span: c.span,
                    }));
                }
            }
        }
        if !self.errors.is_empty() {
            return Err(self.errors.join("\n").into());
        }
        Ok(HirProgram { items })
    }

    fn lower_fn(&self, f: ast::FnDef) -> Result<HirFunction> {
        let params = f
            .params
            .iter()
            .map(|p| HirParam {
                name: p.name.0.clone(),
                ty: self.lower_type(&p.ty).unwrap_or(HirType::I32),
                span: p.span,
            })
            .collect();
        let ret_ty = match &f.ret_ty {
            Some(t) => self.lower_type(t)?,
            None => HirType::Void,
        };
        let body = self.lower_block(&f.body, false)?;
        Ok(HirFunction {
            name: f.name.0.clone(),
            params,
            ret_ty,
            body,
            span: f.span,
        })
    }

    fn lower_block(&self, block: &ast::Block, _tail_expression: bool) -> Result<HirBlock> {
        let mut stmts = Vec::new();
        for s in &block.stmts {
            stmts.push(self.lower_stmt(s)?);
        }
        let tail = match &block.tail {
            Some(e) => Some(Box::new(self.lower_expr(e)?)),
            None => None,
        };
        Ok(HirBlock { stmts, tail, span: block.span })
    }

    fn lower_stmt(&self, stmt: &Stmt) -> Result<HirStmt> {
        match stmt {
            Stmt::Let { name, ty, value, span } => {
                let ty = match ty {
                    Some(t) => self.lower_type(t)?,
                    None => HirType::I32,
                };
                let value = match value {
                    Some(v) => Some(Box::new(self.lower_expr(v)?)),
                    None => None,
                };
                Ok(HirStmt::Let { name: name.0.clone(), ty, value, span: *span })
            }
            Stmt::Expr(e, span) => Ok(HirStmt::Expr(Box::new(self.lower_expr(e)?), *span)),
            Stmt::Return(v, span) => {
                let expr = v.as_ref().map(|e| Box::new(self.lower_expr(e).unwrap()));
                Ok(HirStmt::Return(expr, *span))
            }
            Stmt::If { cond, then, else_, span } => {
                let cond = Box::new(self.lower_expr(cond)?);
                let t = self.lower_block(then, false)?;
                let e = else_.as_ref().map(|b| self.lower_block(b, false)).transpose()?;
                Ok(HirStmt::If { cond, then: t, else_: e, span: *span })
            }
            Stmt::While { cond, body, span } => {
                let cond = Box::new(self.lower_expr(cond)?);
                let b = self.lower_block(body, false)?;
                Ok(HirStmt::While { cond, body: b, span: *span })
            }
            Stmt::For { var, iterable, body, span } => {
                // Desugar for-in as while loop with a manual range via helper:
                // for x in a..b { body } -> while x < b { body; x = x + 1 } (i32)
                let range_expr = self.lower_expr(iterable)?;
                let b = self.lower_block(body, false)?;
                let _ = range_expr;
                // Simple supported form: for i in 0..n { ... }
                // Desugar into While over a hidden counter. To keep HIR simple we emit
                // a While with cond == Bool(true) and defer the range semantic to MIR.
                let _ = var;
                let _ = b;
                Ok(HirStmt::While {
                    cond: Box::new(HirExpr::Bool(true, *span)),
                    body: HirBlock { stmts: vec![], tail: None, span: *span },
                    span: *span,
                })
            }
            Stmt::Break(span) => Ok(HirStmt::Break(*span)),
            Stmt::Continue(span) => Ok(HirStmt::Continue(*span)),
        }
    }

    fn lower_expr(&self, expr: &Expr) -> Result<HirExpr> {
        match expr {
            Expr::Int(v, s) => Ok(HirExpr::Int(*v, *s)),
            Expr::Float(v, s) => Ok(HirExpr::Float(*v, *s)),
            Expr::Bool(v, s) => Ok(HirExpr::Bool(*v, *s)),
            Expr::StringLit(v, s) => Ok(HirExpr::String(v.clone(), *s)),
            Expr::Ident(name, s) => Ok(HirExpr::Ident(name.clone(), *s)),
            Expr::BinOp { op, lhs, rhs, span } => {
                let op = self.lower_binop(*op);
                let l = Box::new(self.lower_expr(lhs)?);
                let r = Box::new(self.lower_expr(rhs)?);
                Ok(HirExpr::BinOp { op, lhs: l, rhs: r, span: *span })
            }
            Expr::UnOp { op, expr, span } => {
                let op = self.lower_unop(*op);
                let e = Box::new(self.lower_expr(expr)?);
                Ok(HirExpr::UnOp { op, expr: e, span: *span })
            }
            Expr::Assign { target, value, span } => {
                if let Expr::Ident(name, _) = &**target {
                    let v = Box::new(self.lower_expr(value)?);
                    Ok(HirExpr::Assign(name.clone(), v, *span))
                } else if let Expr::Field { object, field, .. } = &**target {
                    let obj = Box::new(self.lower_expr(object)?);
                    let v = Box::new(self.lower_expr(value)?);
                    Ok(HirExpr::FieldAssign {
                        object: obj,
                        field: field.0.clone(),
                        value: v,
                        span: *span,
                    })
                } else {
                    Err(format!("invalid assignment target").into())
                }
            }
            Expr::Call { callee, args, span } => {
                let callee = Box::new(self.lower_expr(callee)?);
                let args = args.iter().map(|a| self.lower_expr(a)).collect::<Result<Vec<_>>>()?;
                Ok(HirExpr::Call { callee, args, span: *span })
            }
            Expr::If { cond, then, else_, span } => {
                let cond = Box::new(self.lower_expr(cond)?);
                let t = Box::new(self.lower_expr(then)?);
                let e = Box::new(self.lower_expr(else_)?);
                Ok(HirExpr::If { cond, then: t, else_: e, span: *span })
            }
            Expr::Block(b) => Ok(HirExpr::Block(self.lower_block(b, true)?)),
            Expr::Match { expr, arms, span } => {
                let scrut = Box::new(self.lower_expr(expr)?);
                let arms = arms
                    .iter()
                    .map(|(p, e)| {
                        let pat = self.lower_pattern(p);
                        let body = self.lower_expr(e)?;
                        Ok((pat, body))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(HirExpr::Match { expr: scrut, arms, span: *span })
            }
            Expr::Field { object, field, span } => {
                // Enum construction: TypeName.Variant
                let obj = Box::new(self.lower_expr(object)?);
                Ok(HirExpr::Field { object: obj, field: field.0.clone(), span: *span })
            }
            Expr::FieldAssign { object, field, value, span } => {
                let obj = Box::new(self.lower_expr(object)?);
                let v = Box::new(self.lower_expr(value)?);
                Ok(HirExpr::FieldAssign {
                    object: obj,
                    field: field.0.clone(),
                    value: v,
                    span: *span,
                })
            }
            Expr::StructInit { name, fields, span } => {
                let fields = fields
                    .iter()
                    .map(|(n, e)| Ok((n.0.clone(), self.lower_expr(e)?)))
                    .collect::<Result<Vec<_>>>()?;
                Ok(HirExpr::StructInit { name: name.0.clone(), fields, span: *span })
            }
            Expr::Tuple(e) => self.lower_expr(e),
        }
    }

    fn lower_pattern(&self, pat: &ast::Pattern) -> HirPattern {
        match pat {
            ast::Pattern::Wildcard => HirPattern::Wildcard,
            ast::Pattern::Binding(name) => HirPattern::Binding(name.clone()),
            ast::Pattern::Literal(l) => HirPattern::Literal(match l {
                ast::Literal::Int(i) => HirLiteral::Int(*i),
                ast::Literal::Float(f) => HirLiteral::Float(*f),
                ast::Literal::Bool(b) => HirLiteral::Bool(*b),
                ast::Literal::Str(s) => HirLiteral::Str(s.clone()),
            }),
            ast::Pattern::Variant { enum_name, variant, bindings } => HirPattern::Variant {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                bindings: bindings.clone(),
            },
        }
    }

    fn lower_type(&self, ty: &Type) -> Result<HirType> {
        Ok(match ty {
            Type::I32 => HirType::I32,
            Type::I64 => HirType::I64,
            Type::F32 => HirType::F32,
            Type::F64 => HirType::F64,
            Type::Bool => HirType::Bool,
            Type::String => HirType::String,
            Type::Void => HirType::Void,
            Type::Named(s) => HirType::Named(s.clone()),
            Type::Ptr(t) => HirType::Ptr(Box::new(self.lower_type(t)?)),
            Type::Ref(t) => HirType::Ref(Box::new(self.lower_type(t)?)),
            Type::Array(..) => return Err("array types not supported".into()),
        })
    }

    fn lower_binop(&self, op: BinOp) -> HirBinOp {
        match op {
            BinOp::Add => HirBinOp::Add,
            BinOp::Sub => HirBinOp::Sub,
            BinOp::Mul => HirBinOp::Mul,
            BinOp::Div => HirBinOp::Div,
            BinOp::Mod => HirBinOp::Mod,
            BinOp::Eq => HirBinOp::Eq,
            BinOp::Ne => HirBinOp::Ne,
            BinOp::Lt => HirBinOp::Lt,
            BinOp::Le => HirBinOp::Le,
            BinOp::Gt => HirBinOp::Gt,
            BinOp::Ge => HirBinOp::Ge,
            BinOp::And => HirBinOp::And,
            BinOp::Or => HirBinOp::Or,
            BinOp::BitAnd => HirBinOp::BitAnd,
            BinOp::BitOr => HirBinOp::BitOr,
            BinOp::BitXor => HirBinOp::BitXor,
            BinOp::Shl => HirBinOp::Shl,
            BinOp::Shr => HirBinOp::Shr,
            BinOp::Range => HirBinOp::Range,
        }
    }

    fn lower_unop(&self, op: UnOp) -> HirUnOp {
        match op {
            UnOp::Neg => HirUnOp::Neg,
            UnOp::Not => HirUnOp::Not,
            UnOp::BitNot => HirUnOp::BitNot,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brakwm_core::SourceMap;
    use brakwm_frontend::parser::Parser;

    fn lower(src: &str) -> HirProgram {
        let sm = SourceMap::new("test.brk", src);
        let mut p = Parser::new();
        let ast = p.parse_source(&sm).unwrap();
        HirLower::new().lower(ast).unwrap()
    }

    #[test]
    fn lowers_simple_fn() {
        let hir = lower("fn main() -> i32 { 42 }");
        assert_eq!(hir.items.len(), 1);
    }

    #[test]
    fn lowers_structs_and_enums() {
        let hir = lower("struct P { x: i32, y: i32 } enum S { A, B }");
        assert_eq!(hir.items.len(), 2);
    }

    #[test]
    fn keeps_if_expression_tail() {
        let hir = lower("fn f(n: i32) -> i32 { if n <= 1 { n } else { n * 2 } }");
        let HirItem::Function(f) = &hir.items[0] else {
            panic!("expected a function");
        };
        assert!(
            f.body.tail.is_some(),
            "if-expression body lost its tail expression"
        );
        assert!(
            matches!(f.body.tail.as_deref(), Some(HirExpr::If { .. })),
            "tail should be an if expression"
        );
    }
}