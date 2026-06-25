use crate::diagnostics::Span;
use crate::types::Type;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub declarations: Vec<Decl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Decl {
    Import(ImportDecl),
    Function(FunctionDecl),
    Type(TypeDecl),
    Error(ErrorDecl),
    Route(RouteDecl),
}

pub type TypeRef = Type;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDecl {
    pub items: Vec<ImportItem>,
    pub module: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportItem {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDecl {
    pub exported: bool,
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDecl {
    pub exported: bool,
    pub name: String,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDecl {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDecl {
    pub exported: bool,
    pub name: String,
    pub variants: Vec<ErrorVariant>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorVariant {
    pub name: String,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecl {
    pub exported: bool,
    pub method: HttpMethod,
    pub path: String,
    pub params: Vec<RouteField>,
    pub query: Vec<RouteField>,
    pub body_type: Option<TypeRef>,
    pub ok_status: u16,
    pub ok_type: TypeRef,
    pub error_type: Option<TypeRef>,
    pub errors: Vec<RouteErrorMapping>,
    pub effects: Vec<String>,
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteField {
    pub name: String,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteErrorMapping {
    pub name: String,
    pub status: u16,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Stmt {
    Var {
        mutable: bool,
        name: String,
        annotation: Option<Type>,
        expr: Expr,
        span: Span,
    },
    Return {
        expr: Expr,
        span: Span,
    },
    If {
        condition: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
        span: Span,
    },
    Expr {
        expr: Expr,
        span: Span,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    Int {
        value: i64,
        span: Span,
    },
    Float {
        value: f64,
        span: Span,
    },
    String {
        value: String,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    Var {
        name: String,
        span: Span,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
    Call {
        callee: String,
        args: Vec<Expr>,
        span: Span,
    },
    Try {
        expr: Box<Expr>,
        span: Span,
    },
    ObjectLiteral {
        fields: Vec<ObjectField>,
        span: Span,
    },
    StructLiteral {
        name: String,
        fields: Vec<FieldValue>,
        span: Span,
    },
    FieldAccess {
        object: Box<Expr>,
        field: String,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    ErrorVariantLiteral {
        error: String,
        variant: String,
        fields: Vec<FieldValue>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Int { span, .. }
            | Expr::Float { span, .. }
            | Expr::String { span, .. }
            | Expr::Bool { span, .. }
            | Expr::Var { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Call { span, .. }
            | Expr::Try { span, .. }
            | Expr::ObjectLiteral { span, .. }
            | Expr::StructLiteral { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::Unary { span, .. }
            | Expr::ErrorVariantLiteral { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectField {
    Named {
        name: String,
        expr: Expr,
        span: Span,
    },
    Spread {
        expr: Expr,
        span: Span,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldValue {
    pub name: String,
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,
}
