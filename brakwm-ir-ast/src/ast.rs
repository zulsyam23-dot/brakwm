use std::fmt;

use brakwm_core::{combine_hash, ContentHash, Span};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Item {
    FnDef(FnDef),
    ExternFn(ExternFn),
    Let(Let),
    Struct(StructDef),
    Enum(EnumDef),
    Const(ConstDef),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Let {
    pub name: Ident,
    pub ty: Option<Type>,
    pub value: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstDef {
    pub vis: Visibility,
    pub name: Ident,
    pub ty: Type,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructDef {
    pub vis: Visibility,
    pub name: Ident,
    pub fields: Vec<Field>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub vis: Visibility,
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumDef {
    pub vis: Visibility,
    pub name: Ident,
    pub variants: Vec<Variant>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    pub name: Ident,
    pub fields: Option<Vec<Type>>, // tuple variant payloads
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternFn {
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret_ty: Option<Type>,
    pub abi: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FnDef {
    pub vis: Visibility,
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret_ty: Option<Type>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Param {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Stmt {
    Let {
        name: Ident,
        ty: Option<Type>,
        value: Option<Expr>,
        span: Span,
    },
    Expr(Expr, Span),
    Return(Option<Expr>, Span),
    If {
        cond: Expr,
        then: Block,
        else_: Option<Block>,
        span: Span,
    },
    While {
        cond: Expr,
        body: Block,
        span: Span,
    },
    For {
        var: Ident,
        iterable: Expr,
        body: Block,
        span: Span,
    },
    Break(Span),
    Continue(Span),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    Int(i64, Span),
    Float(f64, Span),
    Bool(bool, Span),
    StringLit(String, Span),
    Ident(String, Span),
    BinOp {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    UnOp {
        op: UnOp,
        expr: Box<Expr>,
        span: Span,
    },
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    If {
        cond: Box<Expr>,
        then: Box<Expr>,
        else_: Box<Expr>,
        span: Span,
    },
    Block(Block),
    Match {
        expr: Box<Expr>,
        arms: Vec<(Pattern, Expr)>,
        span: Span,
    },
    Field {
        object: Box<Expr>,
        field: Ident,
        span: Span,
    },
    FieldAssign {
        object: Box<Expr>,
        field: Ident,
        value: Box<Expr>,
        span: Span,
    },
    StructInit {
        name: Ident,
        fields: Vec<(Ident, Expr)>,
        span: Span,
    },
    Tuple(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    Wildcard,
    Binding(String),
    Literal(Literal),
    Variant {
        enum_name: String,
        variant: String,
        bindings: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Range,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Type {
    I32,
    I64,
    F32,
    F64,
    Bool,
    String,
    Void,
    Named(String),
    Ptr(Box<Type>),
    Ref(Box<Type>),
    Array(Box<Type>, usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Ident(pub String);

impl std::ops::Deref for Ident {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl From<String> for Ident {
    fn from(s: String) -> Self {
        Ident(s)
    }
}

impl From<&str> for Ident {
    fn from(s: &str) -> Self {
        Ident(s.to_string())
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Type {
    pub fn to_string(&self) -> String {
        match self {
            Type::I32 => "i32".into(),
            Type::I64 => "i64".into(),
            Type::F32 => "f32".into(),
            Type::F64 => "f64".into(),
            Type::Bool => "bool".into(),
            Type::String => "string".into(),
            Type::Void => "void".into(),
            Type::Named(s) => s.clone(),
            Type::Ptr(t) => format!("{}*", t.to_string()),
            Type::Ref(t) => format!("&{}", t.to_string()),
            Type::Array(t, n) => format!("[{}, {}]", t.to_string(), n),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Int(_, s)
            | Expr::Float(_, s)
            | Expr::Bool(_, s)
            | Expr::StringLit(_, s)
            | Expr::Ident(_, s)
            | Expr::BinOp { span: s, .. }
            | Expr::UnOp { span: s, .. }
            | Expr::Assign { span: s, .. }
            | Expr::Call { span: s, .. }
            | Expr::If { span: s, .. }
            | Expr::Match { span: s, .. }
            | Expr::Field { span: s, .. }
            | Expr::FieldAssign { span: s, .. }
            | Expr::StructInit { span: s, .. } => *s,
            Expr::Block(b) => b.span,
            Expr::Tuple(e) => e.span(),
        }
    }
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for item in &self.items {
            writeln!(f, "{item}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Item::FnDef(x) => write!(f, "{x}"),
            Item::ExternFn(x) => write!(f, "{x}"),
            Item::Let(x) => write!(f, "{x}"),
            Item::Struct(x) => write!(f, "{x}"),
            Item::Enum(x) => write!(f, "{x}"),
            Item::Const(x) => write!(f, "{x}"),
        }
    }
}

impl fmt::Display for FnDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fn {}({}) -> {} {{ ... }}",
            self.name.0,
            self.params
                .iter()
                .map(|p| format!("{}: {}", p.name.0, p.ty))
                .collect::<Vec<_>>()
                .join(", "),
            self.ret_ty.as_ref().map(|t| t.to_string()).unwrap_or_else(|| "void".into())
        )
    }
}

impl fmt::Display for ExternFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "extern \"{}\" fn {}() -> ...", self.abi, self.name.0)
    }
}

impl fmt::Display for StructDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "struct {} {{ ... }}", self.name.0)
    }
}

impl fmt::Display for EnumDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "enum {} {{ ... }}", self.name.0)
    }
}

impl fmt::Display for Let {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "let {}", self.name.0)
    }
}

impl fmt::Display for ConstDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "const {}", self.name.0)
    }
}

impl ContentHash for Program {
    fn content_hash(&self) -> u64 {
        let mut h = 0u64;
        for i in &self.items {
            h = combine_hash(h, i.content_hash());
        }
        h
    }
}

impl ContentHash for Item {
    fn content_hash(&self) -> u64 {
        match self {
            Item::FnDef(x) => combine_hash(1, x.name.0.content_hash()),
            Item::ExternFn(x) => combine_hash(2, x.name.0.content_hash()),
            Item::Let(x) => combine_hash(3, x.name.0.content_hash()),
            Item::Struct(x) => combine_hash(4, x.name.0.content_hash()),
            Item::Enum(x) => combine_hash(5, x.name.0.content_hash()),
            Item::Const(x) => combine_hash(6, x.name.0.content_hash()),
        }
    }
}