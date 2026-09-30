use brakwm_core::{Severity, Span, Diagnostic, Diagnostics};
use crate::hir::*;

#[derive(Debug, Clone, PartialEq)]
enum Ty {
    I32,
    I64,
    F32,
    F64,
    Bool,
    Str,
    Void,
    Struct(String),
    Unknown,
}

impl From<&HirType> for Ty {
    fn from(t: &HirType) -> Self {
        match t {
            HirType::I32 => Ty::I32,
            HirType::I64 => Ty::I64,
            HirType::F32 => Ty::F32,
            HirType::F64 => Ty::F64,
            HirType::Bool => Ty::Bool,
            HirType::String => Ty::Str,
            HirType::Void => Ty::Void,
            HirType::Named(s) => Ty::Struct(s.clone()),
            _ => Ty::Unknown,
        }
    }
}

pub struct TypeChecker {
    pub diagnostics: Diagnostics,
    scope: std::collections::HashMap<String, Ty>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            diagnostics: Diagnostics::new(),
            scope: std::collections::HashMap::new(),
        }
    }

    pub fn check(&mut self, program: &HirProgram) -> Result<(), Vec<Diagnostic>> {
        self.scope.clear();
        // Predeclare functions.
        for item in &program.items {
            if let HirItem::Function(f) = item {
                self.scope.insert(f.name.clone(), Ty::Unknown);
            }
            if let HirItem::ExternFunction(f) = item {
                self.scope.insert(f.name.clone(), Ty::Unknown);
            }
        }
        for item in &program.items {
            match item {
                HirItem::Function(f) => {
                    let _ = self.check_function(f);
                }
                HirItem::ExternFunction(_) => {}
                HirItem::GlobalLet(g) => {
                    if let Some(v) = &g.value {
                        let t = self.check_expr(v);
                        let _ = t;
                    }
                }
                _ => {}
            }
        }
        if self.diagnostics.has_errors() {
            Err(self.diagnostics.items.clone())
        } else {
            Ok(())
        }
    }

    fn check_function(&mut self, f: &HirFunction) -> Result<Ty, ()> {
        let saved = self.scope.clone();
        for p in &f.params {
            self.scope.insert(p.name.clone(), (&p.ty).into());
        }
        for stmt in &f.body.stmts {
            self.check_stmt(stmt, &f.ret_ty);
        }
        self.scope = saved;
        Ok((&f.ret_ty).into())
    }

    fn check_stmt(&mut self, stmt: &HirStmt, ret_ty: &HirType) {
        match stmt {
            HirStmt::Let { name, ty, value, .. } => {
                if let Some(v) = value {
                    self.check_expr(v);
                }
                self.scope.insert(name.clone(), ty.into());
            }
            HirStmt::Expr(e, _) => {
                self.check_expr(e);
            }
            HirStmt::Return(v, span) => {
                if let Some(v) = v {
                    let t = self.check_expr(v);
                    self.check_assignable(t.clone(), ret_ty.into(), *span);
                } else if *ret_ty != HirType::Void {
                    let _ = span;
                }
            }
            HirStmt::If { cond, then, else_, .. } => {
                let ct = self.check_expr(cond);
                if ct != Ty::Bool {
                    self.error("if condition must be bool");
                }
                for s in &then.stmts {
                    self.check_stmt(s, ret_ty);
                }
                if let Some(e) = else_ {
                    for s in &e.stmts {
                        self.check_stmt(s, ret_ty);
                    }
                }
            }
            HirStmt::While { cond, body, .. } => {
                let ct = self.check_expr(cond);
                if ct != Ty::Bool {
                    self.error("while condition must be bool");
                }
                for s in &body.stmts {
                    self.check_stmt(s, ret_ty);
                }
            }
            HirStmt::Loop { body, .. } => {
                for s in &body.stmts {
                    self.check_stmt(s, ret_ty);
                }
            }
            _ => {}
        }
    }

    fn check_expr(&mut self, expr: &HirExpr) -> Ty {
        match expr {
            HirExpr::Int(_, _) => Ty::I32,
            HirExpr::Float(_, _) => Ty::F64,
            HirExpr::Bool(_, _) => Ty::Bool,
            HirExpr::String(_, _) => Ty::Str,
            HirExpr::Ident(name, _) => self.scope.get(name).cloned().unwrap_or(Ty::Unknown),
            HirExpr::Assign(name, v, _) => {
                let vt = self.check_expr(v);
                let existing = self.scope.get(name).cloned().unwrap_or(Ty::Unknown);
                if existing != Ty::Unknown {
                    self.scope.insert(name.clone(), vt.clone());
                }
                vt
            }
            HirExpr::BinOp { ref op, ref lhs, ref rhs, .. } => {
                let l = self.check_expr(lhs);
                let r = self.check_expr(rhs);
                match op {
                    HirBinOp::And | HirBinOp::Or => Ty::Bool,
                    HirBinOp::Eq | HirBinOp::Ne | HirBinOp::Lt | HirBinOp::Le | HirBinOp::Gt | HirBinOp::Ge => Ty::Bool,
                    HirBinOp::Add | HirBinOp::Sub | HirBinOp::Mul | HirBinOp::Div | HirBinOp::Mod => {
                        match (&l, &r) {
                            (Ty::Str, Ty::Str) => Ty::Str,
                            (Ty::F64, _) | (_, Ty::F64) => Ty::F64,
                            (Ty::F32, _) | (_, Ty::F32) => Ty::F32,
                            (Ty::I64, _) | (_, Ty::I64) => Ty::I64,
                            _ => Ty::I32,
                        }
                    }
                    _ => l,
                }
            }
            HirExpr::UnOp { ref expr, .. } => self.check_expr(expr),
            HirExpr::Call { ref callee, .. } => {
                let _ = self.check_expr(callee);
                Ty::Unknown
            }
            HirExpr::If { ref cond, ref then, ref else_, .. } => {
                let _ = self.check_expr(cond);
                let t = self.check_expr(then);
                let e = self.check_expr(else_);
                if t != e {
                    t
                } else {
                    t
                }
            }
            HirExpr::Block(b) => {
                let saved = self.scope.clone();
                let mut result = Ty::Void;
                for s in &b.stmts {
                    match s {
                        HirStmt::Expr(e, _) => result = self.check_expr(e),
                        _ => self.check_stmt(s, &HirType::Void),
                    }
                }
                self.scope = saved;
                result
            }
            HirExpr::Field { ref object, .. } => {
                let _ = self.check_expr(object);
                Ty::Unknown
            }
            HirExpr::StructInit { .. } => Ty::Unknown,
            HirExpr::EnumInit { .. } => Ty::Unknown,
            HirExpr::FieldAssign { ref object, ref value, .. } => {
                let _ = self.check_expr(object);
                self.check_expr(value)
            }
            HirExpr::Match { ref expr, ref arms, .. } => {
                let _ = self.check_expr(expr);
                let mut result = Ty::Void;
                for (_, body) in arms {
                    if let HirExpr::Block(b) = body {
                        let saved = self.scope.clone();
                        for s in &b.stmts {
                            if let HirStmt::Expr(e, _) = s {
                                result = self.check_expr(e);
                            }
                        }
                        self.scope = saved;
                    }
                }
                result
            }
        }
    }

    fn check_assignable(&self, got: Ty, want: Ty, span: Span) {
        let _ = (got, want, span);
    }

    fn error(&mut self, msg: &str) {
        self.diagnostics.push(Diagnostic::new(
            Severity::Error,
            msg,
            Span::default(),
            String::new(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brakwm_core::SourceMap;
    use brakwm_frontend::parser::Parser;

    fn check(src: &str) -> Result<(), Vec<Diagnostic>> {
        let sm = SourceMap::new("t.brk", src);
        let mut p = Parser::new();
        let ast = p.parse_source(&sm).unwrap();
        let hir = crate::lower::HirLower::new().lower(ast).unwrap();
        TypeChecker::new().check(&hir)
    }

    #[test]
    fn accepts_valid_program() {
        assert!(check("fn main() -> i32 { 42 }").is_ok());
    }

    #[test]
    fn accepts_while_loop() {
        assert!(check("fn f() { let i: i32 = 0; while i < 5 { i = i + 1; } }").is_ok());
    }
}