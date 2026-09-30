use brakwm_core::{combine_hash, ContentHash, Span};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HirProgram {
    pub items: Vec<HirItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HirItem {
    Function(HirFunction),
    ExternFunction(HirExternFunction),
    GlobalLet(HirGlobalLet),
    Struct(HirStruct),
    Enum(HirEnum),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HirStruct {
    pub name: String,
    pub fields: Vec<HirField>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HirField {
    pub name: String,
    pub ty: HirType,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HirEnum {
    pub name: String,
    pub variants: Vec<HirVariant>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HirVariant {
    pub name: String,
    pub fields: Option<Vec<HirType>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HirExternFunction {
    pub name: String,
    pub params: Vec<HirParam>,
    pub ret_ty: HirType,
    pub abi: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HirFunction {
    pub name: String,
    pub params: Vec<HirParam>,
    pub ret_ty: HirType,
    pub body: HirBlock,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HirParam {
    pub name: String,
    pub ty: HirType,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HirGlobalLet {
    pub name: String,
    pub ty: HirType,
    pub value: Option<Box<HirExpr>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HirBlock {
    pub stmts: Vec<HirStmt>,
    pub tail: Option<Box<HirExpr>>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HirStmt {
    Let {
        name: String,
        ty: HirType,
        value: Option<Box<HirExpr>>,
        span: Span,
    },
    Expr(Box<HirExpr>, Span),
    Return(Option<Box<HirExpr>>, Span),
    If {
        cond: Box<HirExpr>,
        then: HirBlock,
        else_: Option<HirBlock>,
        span: Span,
    },
    Loop {
        body: HirBlock,
        span: Span,
    },
    While {
        cond: Box<HirExpr>,
        body: HirBlock,
        span: Span,
    },
    Break(Span),
    Continue(Span),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HirExpr {
    Int(i64, Span),
    Float(f64, Span),
    Bool(bool, Span),
    String(String, Span),
    Ident(String, Span),
    Assign(String, Box<HirExpr>, Span),
    BinOp {
        op: HirBinOp,
        lhs: Box<HirExpr>,
        rhs: Box<HirExpr>,
        span: Span,
    },
    UnOp {
        op: HirUnOp,
        expr: Box<HirExpr>,
        span: Span,
    },
    Call {
        callee: Box<HirExpr>,
        args: Vec<HirExpr>,
        span: Span,
    },
    If {
        cond: Box<HirExpr>,
        then: Box<HirExpr>,
        else_: Box<HirExpr>,
        span: Span,
    },
    Block(HirBlock),
    Match {
        expr: Box<HirExpr>,
        arms: Vec<(HirPattern, HirExpr)>,
        span: Span,
    },
    Field {
        object: Box<HirExpr>,
        field: String,
        span: Span,
    },
    StructInit {
        name: String,
        fields: Vec<(String, HirExpr)>,
        span: Span,
    },
    EnumInit {
        enum_name: String,
        variant: String,
        args: Vec<HirExpr>,
        span: Span,
    },
    FieldAssign {
        object: Box<HirExpr>,
        field: String,
        value: Box<HirExpr>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HirPattern {
    Wildcard,
    Binding(String),
    Literal(HirLiteral),
    Variant {
        enum_name: String,
        variant: String,
        bindings: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HirLiteral {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HirBinOp {
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
pub enum HirUnOp {
    Neg,
    Not,
    BitNot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HirType {
    I32,
    I64,
    F32,
    F64,
    Bool,
    String,
    Void,
    Named(String),
    Ptr(Box<HirType>),
    Ref(Box<HirType>),
}

impl HirExpr {
    pub fn span(&self) -> Span {
        match self {
            HirExpr::Int(_, s)
            | HirExpr::Float(_, s)
            | HirExpr::Bool(_, s)
            | HirExpr::String(_, s)
            | HirExpr::Ident(_, s)
            | HirExpr::Assign(_, _, s)
            | HirExpr::BinOp { span: s, .. }
            | HirExpr::UnOp { span: s, .. }
            | HirExpr::Call { span: s, .. }
            | HirExpr::If { span: s, .. }
            | HirExpr::Match { span: s, .. }
            | HirExpr::Field { span: s, .. }
            | HirExpr::StructInit { span: s, .. }
            | HirExpr::EnumInit { span: s, .. }
            | HirExpr::FieldAssign { span: s, .. } => *s,
            HirExpr::Block(b) => b.span,
        }
    }
}

impl std::fmt::Display for HirType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HirType::I32 => write!(f, "i32"),
            HirType::I64 => write!(f, "i64"),
            HirType::F32 => write!(f, "f32"),
            HirType::F64 => write!(f, "f64"),
            HirType::Bool => write!(f, "bool"),
            HirType::String => write!(f, "string"),
            HirType::Void => write!(f, "void"),
            HirType::Named(s) => write!(f, "{s}"),
            HirType::Ptr(t) => write!(f, "{}*", t),
            HirType::Ref(t) => write!(f, "&{}", t),
        }
    }
}

impl std::fmt::Display for HirProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in &self.items {
            writeln!(f, "{item}")?;
        }
        Ok(())
    }
}

impl std::fmt::Display for HirItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HirItem::Function(x) => write!(f, "fn {}(...) -> {} {{ ... }}", x.name, x.ret_ty),
            HirItem::ExternFunction(x) => write!(f, "extern {} {}", x.abi, x.name),
            HirItem::GlobalLet(x) => write!(f, "let {}: {}", x.name, x.ty),
            HirItem::Struct(x) => write!(f, "struct {}", x.name),
            HirItem::Enum(x) => write!(f, "enum {}", x.name),
        }
    }
}

impl ContentHash for HirProgram {
    fn content_hash(&self) -> u64 {
        let mut h = 0u64;
        for i in &self.items {
            h = combine_hash(h, i.content_hash());
        }
        h
    }
}

impl ContentHash for HirItem {
    fn content_hash(&self) -> u64 {
        match self {
            HirItem::Function(x) => combine_hash(1, x.name.content_hash()),
            HirItem::ExternFunction(x) => combine_hash(2, x.name.content_hash()),
            HirItem::GlobalLet(x) => combine_hash(3, x.name.content_hash()),
            HirItem::Struct(x) => combine_hash(4, x.name.content_hash()),
            HirItem::Enum(x) => combine_hash(5, x.name.content_hash()),
        }
    }
}