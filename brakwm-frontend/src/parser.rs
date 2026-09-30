use brakwm_core::{Result, SourceMap, Span, DUMMY_SPAN};
use brakwm_ir_ast::ast::*;
use crate::lexer::{AsciiLexer, BrakLexer, Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    file: String,
}

impl Parser {
    pub fn new() -> Self {
        Self { tokens: Vec::new(), pos: 0, file: String::new() }
    }

    /// Parse source text directly from a `SourceMap`.
    pub fn parse_source(&mut self, sm: &SourceMap) -> Result<Program> {
        self.file = sm.filename().to_string();
        let mut lexer = AsciiLexer::new();
        self.tokens = lexer.lex(sm);
        self.pos = 0;
        self.parse_program()
    }

    /// Parse a token stream produced by any [`BrakLexer`].
    pub fn parse(&mut self, tokens: &[Token]) -> Result<Program> {
        self.tokens = tokens.to_vec();
        self.pos = 0;
        self.parse_program()
    }

    // ---------------- token helpers ----------------

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek2(&self) -> &TokenKind {
        let idx = (self.pos + 1).min(self.tokens.len() - 1);
        &self.tokens[idx].kind
    }

    fn advance(&mut self) -> TokenKind {
        let t = self.tokens[self.pos].kind.clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn at(&self, kind: &TokenKind) -> bool {
        let p = self.peek();
        std::mem::discriminant(p) == std::mem::discriminant(kind)
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<()> {
        if self.at(&kind) {
            self.advance();
            Ok(())
        } else {
            Err(format!(
                "expected {:?}, got {:?}",
                kind,
                self.peek()
            )
            .into())
        }
    }

    fn ident(&mut self) -> Result<String> {
        match self.peek().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(name)
            }
            other => Err(format!("expected identifier, got {other}").into()),
        }
    }

    fn span(&self) -> brakwm_core::Span {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or_else(|| Token {
            kind: TokenKind::Eof,
            lexeme: String::new(),
            offset: 0,
            len: 0,
        });
        let off = tok.offset;
        let sm = SourceMap::new(&self.file, "");
        let _ = sm;
        brakwm_core::Span::new(
            brakwm_core::SourceLoc::new(0, 0, off),
            brakwm_core::SourceLoc::new(0, 0, off),
        )
    }

    // ---------------- program ----------------

    fn parse_program(&mut self) -> Result<Program> {
        let mut items = Vec::new();
        while !self.at(&TokenKind::Eof) {
            items.push(self.parse_item()?);
        }
        Ok(Program { items })
    }

    fn parse_item(&mut self) -> Result<Item> {
        // Visibility
        if self.at(&TokenKind::Ident("pub".into())) {
            self.advance();
        }
        let vis = Visibility::Public;

        let item = match self.peek().clone() {
            TokenKind::Fn => {
                self.advance();
                Item::FnDef(self.parse_fn_body(vis)?)
            }
            TokenKind::Extern => {
                self.advance();
                Item::ExternFn(self.parse_extern_fn()?)
            }
            TokenKind::Let => {
                self.advance();
                let (name, ty, value, span) = self.parse_let_binding()?;
                Item::Let(Let { name, ty, value, span })
            }
            TokenKind::Struct => {
                self.advance();
                Item::Struct(self.parse_struct()?)
            }
            TokenKind::Enum => {
                self.advance();
                Item::Enum(self.parse_enum()?)
            }
            TokenKind::Const => {
                self.advance();
                let name: Ident = self.ident()?.into();
                self.expect(TokenKind::Colon)?;
                let ty = self.parse_type()?;
                self.expect(TokenKind::Assign)?;
                let value = self.parse_expr(0)?;
                self.eat(&TokenKind::Semicolon);
                let span = self.span();
                Item::Const(ConstDef { vis, name, ty, value, span })
            }
            other => {
                return Err(format!("unexpected token {other} at item level").into());
            }
        };
        Ok(item)
    }

    fn parse_fn_body(&mut self, vis: Visibility) -> Result<FnDef> {
        let name: Ident = self.ident()?.into();
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen)?;
        let ret_ty = if self.at(&TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        let span = self.span();
        Ok(FnDef { vis, name, params, ret_ty, body, span })
    }

    fn parse_extern_fn(&mut self) -> Result<ExternFn> {
        let abi = if self.at(&TokenKind::Ident("C".into())) {
            self.advance();
            "C".to_string()
        } else {
            "C".to_string()
        };
        self.expect(TokenKind::Fn)?;
        let name: Ident = self.ident()?.into();
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen)?;
        let ret_ty = if self.at(&TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.eat(&TokenKind::Semicolon);
        let span = self.span();
        Ok(ExternFn { name, params, ret_ty, abi, span })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>> {
        let mut params = Vec::new();
        while !self.at(&TokenKind::RParen) && !self.at(&TokenKind::Eof) {
            let name: Ident = self.ident()?.into();
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            let span = self.span();
            params.push(Param { name, ty, span });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    fn parse_type(&mut self) -> Result<Type> {
        let mut ty = match self.peek().clone() {
            TokenKind::Type(t) => {
                self.advance();
                t
            }
            TokenKind::Ident(name) => {
                let n = name.clone();
                self.advance();
                Type::Named(n)
            }
            TokenKind::LBracket => {
                self.advance();
                let inner = self.parse_type()?;
                self.expect(TokenKind::Comma)?;
                let n = match self.peek().clone() {
                    TokenKind::Int(v) => {
                        self.advance();
                        v as usize
                    }
                    _ => 0,
                };
                self.expect(TokenKind::RBracket)?;
                Type::Array(Box::new(inner), n)
            }
            other => {
                return Err(format!("expected type, got {other}").into());
            }
        };
        // pointer types: `i32*`
        while self.at(&TokenKind::Star) {
            self.advance();
            ty = Type::Ptr(Box::new(ty));
        }
        Ok(ty)
    }

    fn parse_struct(&mut self) -> Result<StructDef> {
        let name: Ident = self.ident()?.into();
        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            let vis = Visibility::Private;
            let fname: Ident = self.ident()?.into();
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            let span = self.span();
            fields.push(Field { vis, name: fname, ty, span });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RBrace)?;
        let span = self.span();
        Ok(StructDef { vis: Visibility::Private, name, fields, span })
    }

    fn parse_enum(&mut self) -> Result<EnumDef> {
        let name: Ident = self.ident()?.into();
        self.expect(TokenKind::LBrace)?;
        let mut variants = Vec::new();
        while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            let vname: Ident = self.ident()?.into();
            let fields = if self.at(&TokenKind::LParen) {
                self.advance();
                let mut tys = Vec::new();
                while !self.at(&TokenKind::RParen) && !self.at(&TokenKind::Eof) {
                    tys.push(self.parse_type()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RParen)?;
                Some(tys)
            } else {
                None
            };
            let span = self.span();
            variants.push(Variant { name: vname, fields, span });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RBrace)?;
        let span = self.span();
        Ok(EnumDef { vis: Visibility::Private, name, variants, span })
    }

    // ---------------- statements & blocks ----------------

    fn parse_block(&mut self) -> Result<Block> {
        self.expect(TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        // A trailing expression without a semicolon is the block's value.
        let mut tail: Option<Box<Expr>> = None;
        loop {
            if self.at(&TokenKind::RBrace) || self.at(&TokenKind::Eof) {
                break;
            }
            if self.is_tail_expr_start() {
                // Speculative: an expression statement like `i = i + 1` also
                // starts here, but is followed by `=` rather than `}`.
                let save = self.pos;
                match self.parse_expr(0) {
                    Ok(expr) if self.at(&TokenKind::RBrace) || self.at(&TokenKind::Eof) => {
                        tail = Some(Box::new(expr));
                        break;
                    }
                    Ok(_) => {
                        self.pos = save;
                    }
                    Err(_) => {
                        self.pos = save;
                    }
                }
            }
            stmts.push(self.parse_stmt()?);
        }
        self.expect(TokenKind::RBrace)?;
        let span = self.span();
        Ok(Block { stmts, tail, span })
    }

    /// True when the upcoming token can start an expression that is allowed to
    /// be a block's tail value. `if` is included because the speculative parse
    /// in `parse_block` rewinds to a statement if the value is not the last
    /// expression in the block.
    fn is_tail_expr_start(&self) -> bool {
        !matches!(
            self.peek(),
            TokenKind::Let
                | TokenKind::Return
                | TokenKind::While
                | TokenKind::For
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Semicolon
                | TokenKind::RBrace
                | TokenKind::Eof
        )
    }

    fn parse_let_binding(&mut self) -> Result<(Ident, Option<Type>, Option<Expr>, Span)> {
        // `let mut x = ...` — mutability is accepted but not enforced yet.
        self.eat(&TokenKind::Mut);
        let name: Ident = self.ident()?.into();
        if self.at(&TokenKind::Colon) {
            self.advance();
            let ty = self.parse_type()?;
            self.expect(TokenKind::Assign)?;
            let value = self.parse_expr(0)?;
            self.eat(&TokenKind::Semicolon);
            let span = self.span();
            Ok((name, Some(ty), Some(value), span))
        } else {
            self.expect(TokenKind::Assign)?;
            let value = self.parse_expr(0)?;
            self.eat(&TokenKind::Semicolon);
            let span = self.span();
            Ok((name, None, Some(value), span))
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt> {
        match self.peek().clone() {
            TokenKind::Let => {
                self.advance();
                let (name, ty, value, span) = self.parse_let_binding()?;
                Ok(Stmt::Let { name, ty, value, span })
            }
            TokenKind::Return => {
                self.advance();
                let value = if self.at(&TokenKind::Semicolon) {
                    None
                } else {
                    Some(self.parse_expr(0)?)
                };
                self.eat(&TokenKind::Semicolon);
                let span = self.span();
                Ok(Stmt::Return(value, span))
            }
            TokenKind::If => {
                self.advance();
                self.parse_if_stmt()
            }
            TokenKind::While => {
                self.advance();
                // Condition parentheses are optional: `while i < 5 { .. }`.
                let cond = if self.eat(&TokenKind::LParen) {
                    let c = self.parse_expr(0)?;
                    self.expect(TokenKind::RParen)?;
                    c
                } else {
                    self.parse_expr(0)?
                };
                let body = self.parse_block()?;
                let span = self.span();
                Ok(Stmt::While { cond, body, span })
            }
            TokenKind::For => {
                self.advance();
                let var: Ident = self.ident()?.into();
                self.expect(TokenKind::In)?;
                let iterable = self.parse_expr(0)?;
                let body = self.parse_block()?;
                let span = self.span();
                Ok(Stmt::For { var, iterable, body, span })
            }
            TokenKind::Break => {
                self.advance();
                self.eat(&TokenKind::Semicolon);
                Ok(Stmt::Break(self.span()))
            }
            TokenKind::Continue => {
                self.advance();
                self.eat(&TokenKind::Semicolon);
                Ok(Stmt::Continue(self.span()))
            }
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                let span = self.span();
                Ok(Stmt::Expr(Expr::Block(block), span))
            }
            _ => {
                let expr = self.parse_expr(0)?;
                let span = self.span();
                self.eat(&TokenKind::Semicolon);
                Ok(Stmt::Expr(expr, span))
            }
        }
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt> {
        // Condition parentheses are optional here too.
        let cond = if self.eat(&TokenKind::LParen) {
            let c = self.parse_expr(0)?;
            self.expect(TokenKind::RParen)?;
            c
        } else {
            self.parse_expr(0)?
        };
        let then = self.parse_block()?;
        let else_ = if self.at(&TokenKind::Else) && self.peek2() == &TokenKind::If {
            // else if
            self.advance();
            self.advance();
            // Represent as nested If statement wrapped in a block
            let mut nested = Vec::new();
            nested.push(self.parse_if_stmt()?);
            Some(Block { stmts: nested, tail: None, span: then.span })
        } else if self.eat(&TokenKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };
        let span = self.span();
        Ok(Stmt::If { cond, then, else_, span })
    }

    // ---------------- expressions (Pratt) ----------------

    const BINDING_POWER: &'static [(TokenKind, (u8, u8))] = &[
        (TokenKind::OrOr, (1, 2)),
        (TokenKind::AndAnd, (3, 4)),
        (TokenKind::Eq, (5, 6)),
        (TokenKind::Ne, (5, 6)),
        (TokenKind::Lt, (7, 8)),
        (TokenKind::Le, (7, 8)),
        (TokenKind::Gt, (7, 8)),
        (TokenKind::Ge, (7, 8)),
        (TokenKind::Shl, (9, 10)),
        (TokenKind::Shr, (9, 10)),
        (TokenKind::Plus, (11, 12)),
        (TokenKind::Minus, (11, 12)),
        (TokenKind::Star, (13, 14)),
        (TokenKind::Slash, (13, 14)),
        (TokenKind::Percent, (13, 14)),
        (TokenKind::Amp, (9, 10)),
        (TokenKind::Pipe, (9, 10)),
        (TokenKind::Caret, (9, 10)),
        (TokenKind::Range, (0, 1)),
    ];

    fn infix_bp(&self, kind: &TokenKind) -> Option<(u8, u8)> {
        for (k, bp) in Self::BINDING_POWER {
            if std::mem::discriminant(k) == std::mem::discriminant(kind) {
                return Some(*bp);
            }
        }
        None
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr> {
        let mut lhs = self.parse_prefix()?;

        loop {
            // postfix: call and field access bind tighter than everything
            if self.at(&TokenKind::LParen) {
                self.advance();
                let mut args = Vec::new();
                while !self.at(&TokenKind::RParen) && !self.at(&TokenKind::Eof) {
                    args.push(self.parse_expr(0)?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RParen)?;
                lhs = Expr::Call {
                    callee: Box::new(lhs),
                    args,
                    span: self.span(),
                };
                continue;
            }

            if self.at(&TokenKind::Dot) {
                self.advance();
                let field = self.ident()?.into();
                lhs = Expr::Field {
                    object: Box::new(lhs),
                    field,
                    span: self.span(),
                };
                continue;
            }

            // assignment (right-assoc)
            if self.at(&TokenKind::Assign) {
                if min_bp <= 0 {
                    self.advance();
                    let value = self.parse_expr(0)?;
                    lhs = Expr::Assign {
                        target: Box::new(lhs),
                        value: Box::new(value),
                        span: self.span(),
                    };
                }
                continue;
            }

            let Some((l_bp, r_bp)) = self.infix_bp(self.peek()) else {
                break;
            };
            if l_bp < min_bp {
                break;
            }
            let op = match self.peek().clone() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                TokenKind::Eq => BinOp::Eq,
                TokenKind::Ne => BinOp::Ne,
                TokenKind::Lt => BinOp::Lt,
                TokenKind::Le => BinOp::Le,
                TokenKind::Gt => BinOp::Gt,
                TokenKind::Ge => BinOp::Ge,
                TokenKind::AndAnd => BinOp::And,
                TokenKind::OrOr => BinOp::Or,
                TokenKind::Amp => BinOp::BitAnd,
                TokenKind::Pipe => BinOp::BitOr,
                TokenKind::Caret => BinOp::BitXor,
                TokenKind::Shl => BinOp::Shl,
                TokenKind::Shr => BinOp::Shr,
                TokenKind::Range => BinOp::Range,
                _ => unreachable!(),
            };
            self.advance();
            let rhs = self.parse_expr(r_bp)?;
            lhs = Expr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span: self.span(),
            };
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr> {
        match self.peek().clone() {
            TokenKind::Int(v) => {
                self.advance();
                Ok(Expr::Int(v, self.span()))
            }
            TokenKind::Float(f) => {
                self.advance();
                Ok(Expr::Float(f, self.span()))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Bool(true, self.span()))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Bool(false, self.span()))
            }
            TokenKind::Str(s) => {
                self.advance();
                Ok(Expr::StringLit(s, self.span()))
            }
            TokenKind::Ident(name) => {
                self.advance();
                Ok(Expr::Ident(name, self.span()))
            }
            TokenKind::LParen => {
                self.advance();
                let inner = self.parse_expr(0)?;
                self.expect(TokenKind::RParen)?;
                Ok(Expr::Tuple(Box::new(inner)))
            }
            TokenKind::Minus => {
                self.advance();
                let e = self.parse_expr(13)?;
                Ok(Expr::UnOp { op: UnOp::Neg, expr: Box::new(e), span: self.span() })
            }
            TokenKind::Bang => {
                self.advance();
                let e = self.parse_expr(13)?;
                Ok(Expr::UnOp { op: UnOp::Not, expr: Box::new(e), span: self.span() })
            }
            TokenKind::Tilde => {
                self.advance();
                let e = self.parse_expr(13)?;
                Ok(Expr::UnOp { op: UnOp::BitNot, expr: Box::new(e), span: self.span() })
            }
            TokenKind::If => {
                self.advance();
                // Condition parentheses are optional in expression position:
                // `if 3 > 2 { .. } else { .. }`.
                let cond = if self.eat(&TokenKind::LParen) {
                    let c = self.parse_expr(0)?;
                    self.expect(TokenKind::RParen)?;
                    c
                } else {
                    self.parse_expr(0)?
                };
                let then = self.parse_block()?;
                let else_ = if self.eat(&TokenKind::Else) {
                    if self.at(&TokenKind::If) {
                        let nested = self.parse_prefix()?;
                        Expr::Block(Block { stmts: Vec::new(), tail: Some(Box::new(nested)), span: then.span })
                    } else {
                        Expr::Block(self.parse_block()?)
                    }
                } else {
                    Expr::Block(Block { stmts: Vec::new(), tail: None, span: DUMMY_SPAN })
                };
                Ok(Expr::If {
                    cond: Box::new(cond),
                    then: Box::new(Expr::Block(then)),
                    else_: Box::new(else_),
                    span: self.span(),
                })
            }
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Expr::Block(block))
            }
            TokenKind::Match => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let expr = self.parse_expr(0)?;
                self.expect(TokenKind::RParen)?;
                self.expect(TokenKind::LBrace)?;
                let mut arms = Vec::new();
                while !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
                    let pat = self.parse_pattern()?;
                    self.expect(TokenKind::FatArrow)?;
                    let body = self.parse_expr(0)?;
                    arms.push((pat, body));
                    self.eat(&TokenKind::Comma);
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Expr::Match {
                    expr: Box::new(expr),
                    arms,
                    span: self.span(),
                })
            }
            other => Err(format!("unexpected token {other} in expression").into()),
        }
    }

    fn parse_pattern(&mut self) -> Result<Pattern> {
        match self.peek().clone() {
            TokenKind::Underscore => {
                self.advance();
                Ok(Pattern::Wildcard)
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                // `Enum.Name` variant pattern
                if self.at(&TokenKind::Dot) {
                    self.advance();
                    let variant = self.ident()?;
                    let mut bindings = Vec::new();
                    if self.at(&TokenKind::LParen) {
                        self.advance();
                        while !self.at(&TokenKind::RParen) && !self.at(&TokenKind::Eof) {
                            bindings.push(self.ident()?);
                            if !self.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                        self.expect(TokenKind::RParen)?;
                    }
                    Ok(Pattern::Variant { enum_name: name, variant, bindings })
                } else {
                    Ok(Pattern::Binding(name))
                }
            }
            other => Err(format!("unexpected token {other} in pattern").into()),
        }
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} '{}'", self.kind, self.lexeme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brakwm_core::SourceMap;

    fn parse(src: &str) -> Program {
        let sm = SourceMap::new("test.brk", src);
        Parser::new().parse_source(&sm).unwrap()
    }

    #[test]
    fn parses_empty_fn() {
        let p = parse("fn main() -> i32 { 42 }");
        assert_eq!(p.items.len(), 1);
    }

    #[test]
    fn parses_math_expr() {
        let p = parse("fn add(a: i32, b: i32) -> i32 { a + b }");
        assert_eq!(p.items.len(), 1);
    }

    #[test]
    fn parses_while_and_let() {
        let p = parse("fn f() { let i: i32 = 0; while i < 5 { i = i + 1; } }");
        assert_eq!(p.items.len(), 1);
    }

    #[test]
    fn parses_call() {
        let p = parse("fn main() -> i32 { add(10, 20) }");
        assert_eq!(p.items.len(), 1);
    }
}