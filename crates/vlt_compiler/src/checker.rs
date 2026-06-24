use crate::ast::*;
use crate::diagnostics::{Diagnostic, DiagnosticBag, SourceFile, Span};
use crate::types::Type;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct FunctionSig {
    pub params: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug, Clone)]
pub struct StructSig {
    pub fields: HashMap<String, Type>,
}

#[derive(Debug, Clone, Default)]
pub struct ExternalSymbols {
    pub functions: HashMap<String, FunctionSig>,
    pub structs: HashMap<String, StructSig>,
}

impl ExternalSymbols {
    pub fn insert_function(
        &mut self,
        name: impl Into<String>,
        params: Vec<Type>,
        return_type: Type,
    ) {
        self.functions.insert(
            name.into(),
            FunctionSig {
                params,
                return_type,
            },
        );
    }

    pub fn insert_struct(&mut self, name: impl Into<String>, fields: HashMap<String, Type>) {
        self.structs.insert(name.into(), StructSig { fields });
    }
}

pub fn check_program(program: &Program, source: &SourceFile) -> Result<(), DiagnosticBag> {
    Checker::new(program, source, ExternalSymbols::default(), true).check()
}

pub fn check_program_with_imports(
    program: &Program,
    source: &SourceFile,
    external: ExternalSymbols,
    require_entrypoint: bool,
) -> Result<(), DiagnosticBag> {
    Checker::new(program, source, external, require_entrypoint).check()
}

struct Checker<'a> {
    program: &'a Program,
    source: &'a SourceFile,
    diagnostics: DiagnosticBag,
    functions: HashMap<String, FunctionSig>,
    structs: HashMap<String, StructSig>,
    route_scope: bool,
    require_entrypoint: bool,
}

impl<'a> Checker<'a> {
    fn new(
        program: &'a Program,
        source: &'a SourceFile,
        external: ExternalSymbols,
        require_entrypoint: bool,
    ) -> Self {
        Self {
            program,
            source,
            diagnostics: DiagnosticBag::new(),
            functions: external.functions,
            structs: external.structs,
            route_scope: false,
            require_entrypoint,
        }
    }

    fn check(mut self) -> Result<(), DiagnosticBag> {
        self.collect_declarations();

        for declaration in &self.program.declarations {
            match declaration {
                Decl::Import(_) => {}
                Decl::Function(function) => self.check_function(function),
                Decl::Type(type_decl) => self.check_type_decl(type_decl),
                Decl::Route(route) => self.check_route(route),
            }
        }

        let has_route = self
            .program
            .declarations
            .iter()
            .any(|declaration| matches!(declaration, Decl::Route(_)));

        if self.require_entrypoint && !self.functions.contains_key("main") && !has_route {
            self.error(
                "E100",
                "missing `main` function",
                Span::new(0, 0),
                "add `function main(): void { ... }`",
            );
        }

        if self.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(self.diagnostics)
        }
    }

    fn collect_declarations(&mut self) {
        for declaration in &self.program.declarations {
            match declaration {
                Decl::Import(_) => {}
                Decl::Function(function) => {
                    if self.functions.contains_key(&function.name) {
                        self.error(
                            "E101",
                            format!("duplicate function `{}`", function.name),
                            function.span,
                            "function names must be unique",
                        );
                    }
                    self.functions.insert(
                        function.name.clone(),
                        FunctionSig {
                            params: function
                                .params
                                .iter()
                                .map(|param| param.ty.clone())
                                .collect(),
                            return_type: function.return_type.clone(),
                        },
                    );
                }
                Decl::Type(type_decl) => {
                    if self.structs.contains_key(&type_decl.name) {
                        self.error(
                            "E102",
                            format!("duplicate type `{}`", type_decl.name),
                            type_decl.span,
                            "type names must be unique",
                        );
                    }
                    self.structs.insert(
                        type_decl.name.clone(),
                        StructSig {
                            fields: type_decl
                                .fields
                                .iter()
                                .map(|field| (field.name.clone(), field.ty.clone()))
                                .collect(),
                        },
                    );
                }
                Decl::Route(_) => {}
            }
        }
    }

    fn check_route(&mut self, route: &RouteDecl) {
        self.check_route_status(route);
        self.check_route_path_params(route);

        for field in &route.params {
            self.check_type_exists(&field.ty, field.span);
        }
        for field in &route.query {
            self.check_type_exists(&field.ty, field.span);
        }
        if let Some(body_type) = &route.body_type {
            self.check_type_exists(body_type, route.span);
        }
        self.check_type_exists(&route.ok_type, route.span);

        let params_type = format!("__route_params_{}", route.span.start);
        let query_type = format!("__route_query_{}", route.span.start);
        self.structs.insert(
            params_type.clone(),
            StructSig {
                fields: route
                    .params
                    .iter()
                    .map(|field| (field.name.clone(), field.ty.clone()))
                    .collect(),
            },
        );
        self.structs.insert(
            query_type.clone(),
            StructSig {
                fields: route
                    .query
                    .iter()
                    .map(|field| (field.name.clone(), field.ty.clone()))
                    .collect(),
            },
        );

        let mut env = HashMap::new();
        env.insert("params".to_string(), Type::Struct(params_type));
        env.insert("query".to_string(), Type::Struct(query_type));
        env.insert("ctx".to_string(), Type::Unknown);
        if let Some(body_type) = &route.body_type {
            env.insert("body".to_string(), body_type.clone());
        }

        let return_type = Type::Result(Box::new(route.ok_type.clone()), Box::new(Type::Unknown));
        let previous_route_scope = self.route_scope;
        self.route_scope = true;
        for stmt in &route.statements {
            self.check_stmt(stmt, &return_type, &mut env);
        }
        self.route_scope = previous_route_scope;
    }

    fn check_route_status(&mut self, route: &RouteDecl) {
        if !(100..=599).contains(&route.ok_status) {
            self.error(
                "EHTTP004",
                format!("invalid success status `{}`", route.ok_status),
                route.span,
                "`ok` status must be between 100 and 599",
            );
        }

        for error in &route.errors {
            if !(400..=599).contains(&error.status) {
                self.error(
                    "EHTTP005",
                    format!(
                        "invalid error status `{}` for `{}`",
                        error.status, error.name
                    ),
                    error.span,
                    "route error statuses must be between 400 and 599",
                );
            }
        }

        if matches!(route.method, HttpMethod::Get) && route.body_type.is_some() {
            self.error(
                "EHTTP003",
                "GET routes should not declare a request body",
                route.span,
                "move inputs into `params` or `query` for GET routes",
            );
        }
    }

    fn check_route_path_params(&mut self, route: &RouteDecl) {
        let path_params = path_params(&route.path);
        let declared = route
            .params
            .iter()
            .map(|field| field.name.clone())
            .collect::<HashSet<_>>();

        for path_param in &path_params {
            if !declared.contains(path_param) {
                self.error(
                    "EHTTP001",
                    format!("route path param `{{{path_param}}}` is missing from params block"),
                    route.span,
                    "declare it in `params { ... }`",
                );
            }
        }

        for field in &route.params {
            if !path_params.contains(&field.name) {
                self.error(
                    "EHTTP002",
                    format!("params field `{}` does not exist in route path", field.name),
                    field.span,
                    "remove the field or add the matching `{name}` segment to the path",
                );
            }
        }
    }

    fn check_type_decl(&mut self, type_decl: &TypeDecl) {
        let mut seen = HashSet::new();
        for field in &type_decl.fields {
            if !seen.insert(field.name.clone()) {
                self.error(
                    "E103",
                    format!("duplicate field `{}`", field.name),
                    field.span,
                    "field names must be unique within a type",
                );
            }
            self.check_type_exists(&field.ty, field.span);
        }
    }

    fn check_function(&mut self, function: &FunctionDecl) {
        self.check_type_exists(&function.return_type, function.span);

        let mut env = HashMap::new();
        for param in &function.params {
            self.check_type_exists(&param.ty, param.span);
            if env.insert(param.name.clone(), param.ty.clone()).is_some() {
                self.error(
                    "E104",
                    format!("duplicate parameter `{}`", param.name),
                    param.span,
                    "parameter names must be unique",
                );
            }
        }

        for stmt in &function.body {
            self.check_stmt(stmt, &function.return_type, &mut env);
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, return_type: &Type, env: &mut HashMap<String, Type>) {
        match stmt {
            Stmt::Var {
                name,
                annotation,
                expr,
                span,
                ..
            } => {
                if let Some(annotation) = annotation {
                    self.check_type_exists(annotation, *span);
                    let actual = self.check_expr(expr, env, Some(annotation));
                    self.expect_type(annotation, &actual, expr.span());
                    env.insert(name.clone(), annotation.clone());
                } else {
                    let actual = self.check_expr(expr, env, None);
                    env.insert(name.clone(), materialize_inferred(actual));
                }
            }
            Stmt::Return { expr, .. } => {
                if matches!(return_type, Type::Void) {
                    self.error(
                        "E105",
                        "void function cannot return a value",
                        expr.span(),
                        "remove this return value",
                    );
                } else {
                    let actual = self.check_expr(expr, env, Some(return_type));
                    self.expect_type(return_type, &actual, expr.span());
                }
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                let actual = self.check_expr(condition, env, Some(&Type::Bool));
                self.expect_type(&Type::Bool, &actual, condition.span());

                let mut then_env = env.clone();
                for stmt in then_body {
                    self.check_stmt(stmt, return_type, &mut then_env);
                }
                let mut else_env = env.clone();
                for stmt in else_body {
                    self.check_stmt(stmt, return_type, &mut else_env);
                }
            }
            Stmt::Expr { expr, .. } => {
                self.check_expr(expr, env, None);
            }
        }
    }

    fn check_expr(
        &mut self,
        expr: &Expr,
        env: &HashMap<String, Type>,
        expected: Option<&Type>,
    ) -> Type {
        match expr {
            Expr::Int { .. } => Type::InferInt,
            Expr::Float { .. } => Type::InferFloat,
            Expr::String { .. } => Type::String,
            Expr::Bool { .. } => Type::Bool,
            Expr::Var { name, span } => env.get(name).cloned().unwrap_or_else(|| {
                self.error(
                    "E106",
                    format!("unknown variable `{name}`"),
                    *span,
                    "declare it with `const` or `let` before use",
                );
                Type::Unknown
            }),
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => {
                let left_ty = self.check_expr(left, env, None);
                let right_ty = self.check_expr(right, env, None);
                self.check_binary(*op, &left_ty, &right_ty, *span)
            }
            Expr::Call { callee, args, span } => {
                self.check_call(callee, args, *span, env, expected)
            }
            Expr::Try { expr, .. } => {
                let actual = self.check_expr(expr, env, None);
                match actual {
                    Type::Result(ok, _) => *ok,
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.error(
                            "E124",
                            format!("`try` requires a Result value, found `{other}`"),
                            expr.span(),
                            "call a function that returns `Result<T, E>`",
                        );
                        Type::Unknown
                    }
                }
            }
            Expr::ObjectLiteral { fields, .. } => {
                for field in fields {
                    match field {
                        ObjectField::Named { expr, .. } | ObjectField::Spread { expr, .. } => {
                            self.check_expr(expr, env, None);
                        }
                    }
                }
                Type::Unknown
            }
            Expr::StructLiteral { name, fields, span } => {
                self.check_struct_literal(name, fields, *span, env)
            }
            Expr::FieldAccess {
                object,
                field,
                span,
            } => {
                let object_ty = self.check_expr(object, env, None);
                match object_ty {
                    Type::Struct(name) => {
                        let Some(struct_sig) = self.structs.get(&name).cloned() else {
                            self.error(
                                "E107",
                                format!("unknown type `{name}`"),
                                object.span(),
                                "declare the type before using it",
                            );
                            return Type::Unknown;
                        };
                        match struct_sig.fields.get(field) {
                            Some(field_ty) => field_ty.clone(),
                            None => {
                                self.error(
                                    "E108",
                                    format!("type `{name}` has no field `{field}`"),
                                    *span,
                                    "use a declared field name",
                                );
                                Type::Unknown
                            }
                        }
                    }
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.error(
                            "E109",
                            format!("cannot access field `{field}` on `{other}`"),
                            *span,
                            "field access is only supported on declared object types",
                        );
                        Type::Unknown
                    }
                }
            }
        }
    }

    fn check_binary(&mut self, op: BinaryOp, left: &Type, right: &Type, span: Span) -> Type {
        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                if left.is_numeric() && right.is_numeric() {
                    numeric_result(left, right)
                } else {
                    self.error(
                        "E110",
                        format!("operator requires numeric operands, found `{left}` and `{right}`"),
                        span,
                        "use numeric values on both sides",
                    );
                    Type::Unknown
                }
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                if left.is_assignable_from(right) || right.is_assignable_from(left) {
                    Type::Bool
                } else {
                    self.error(
                        "E111",
                        format!("cannot compare `{left}` with `{right}`"),
                        span,
                        "both sides of equality must have compatible types",
                    );
                    Type::Bool
                }
            }
            BinaryOp::Lt | BinaryOp::Gt | BinaryOp::LtEq | BinaryOp::GtEq => {
                if left.is_numeric() && right.is_numeric() {
                    Type::Bool
                } else {
                    self.error(
                        "E112",
                        format!(
                            "comparison requires numeric operands, found `{left}` and `{right}`"
                        ),
                        span,
                        "use numeric values on both sides",
                    );
                    Type::Bool
                }
            }
        }
    }

    fn check_call(
        &mut self,
        callee: &str,
        args: &[Expr],
        span: Span,
        env: &HashMap<String, Type>,
        expected: Option<&Type>,
    ) -> Type {
        match callee {
            "print" => {
                if args.len() != 1 {
                    self.error(
                        "E113",
                        "`print` expects one argument",
                        span,
                        "call `print(value)`",
                    );
                } else {
                    self.check_expr(&args[0], env, None);
                }
                Type::Void
            }
            "ok" => {
                if args.len() != 1 {
                    self.error(
                        "E114",
                        "`ok` expects one argument",
                        span,
                        "call `ok(value)`",
                    );
                    return Type::Unknown;
                }
                let ok_expected = match expected {
                    Some(Type::Result(ok, _)) => Some(ok.as_ref()),
                    _ => None,
                };
                let ok_ty = self.check_expr(&args[0], env, ok_expected);
                Type::Result(Box::new(ok_ty), Box::new(Type::Unknown))
            }
            "err" => {
                if args.len() != 1 {
                    self.error(
                        "E115",
                        "`err` expects one argument",
                        span,
                        "call `err(value)`",
                    );
                    return Type::Unknown;
                }
                let err_expected = match expected {
                    Some(Type::Result(_, err)) => Some(err.as_ref()),
                    _ => None,
                };
                let err_ty = self.check_expr(&args[0], env, err_expected);
                Type::Result(Box::new(Type::Unknown), Box::new(err_ty))
            }
            _ => {
                let Some(sig) = self.functions.get(callee).cloned() else {
                    if self.route_scope {
                        for arg in args {
                            self.check_expr(arg, env, None);
                        }
                        return Type::Unknown;
                    }
                    self.error(
                        "E116",
                        format!("unknown function `{callee}`"),
                        span,
                        "declare the function before calling it",
                    );
                    return Type::Unknown;
                };

                if args.len() != sig.params.len() {
                    self.error(
                        "E117",
                        format!(
                            "function `{callee}` expects {} arguments, found {}",
                            sig.params.len(),
                            args.len()
                        ),
                        span,
                        "pass the expected number of arguments",
                    );
                    return sig.return_type;
                }

                for (arg, expected) in args.iter().zip(&sig.params) {
                    let actual = self.check_expr(arg, env, Some(expected));
                    self.expect_type(expected, &actual, arg.span());
                }
                sig.return_type
            }
        }
    }

    fn check_struct_literal(
        &mut self,
        name: &str,
        fields: &[FieldValue],
        span: Span,
        env: &HashMap<String, Type>,
    ) -> Type {
        let Some(struct_sig) = self.structs.get(name).cloned() else {
            self.error(
                "E118",
                format!("unknown type `{name}`"),
                span,
                "declare this object type before constructing it",
            );
            return Type::Unknown;
        };

        let mut seen = HashSet::new();
        for field in fields {
            if !seen.insert(field.name.clone()) {
                self.error(
                    "E119",
                    format!("duplicate field `{}` in object literal", field.name),
                    field.span,
                    "provide each field once",
                );
                continue;
            }

            let Some(expected) = struct_sig.fields.get(&field.name) else {
                self.error(
                    "E120",
                    format!("type `{name}` has no field `{}`", field.name),
                    field.span,
                    "use a field declared on the type",
                );
                self.check_expr(&field.expr, env, None);
                continue;
            };

            let actual = self.check_expr(&field.expr, env, Some(expected));
            self.expect_type(expected, &actual, field.expr.span());
        }

        for required in struct_sig.fields.keys() {
            if !seen.contains(required) {
                self.error(
                    "E121",
                    format!("missing field `{required}` for `{name}`"),
                    span,
                    "initialize every field declared by the type",
                );
            }
        }

        Type::Struct(name.to_string())
    }

    fn check_type_exists(&mut self, ty: &Type, span: Span) {
        match ty {
            Type::Struct(name) if !self.structs.contains_key(name) => self.error(
                "E122",
                format!("unknown type `{name}`"),
                span,
                "declare this type before using it",
            ),
            Type::Result(ok, err) => {
                self.check_type_exists(ok, span);
                self.check_type_exists(err, span);
            }
            _ => {}
        }
    }

    fn expect_type(&mut self, expected: &Type, actual: &Type, span: Span) {
        if !expected.is_assignable_from(actual) {
            self.error(
                "E123",
                format!("expected `{expected}`, found `{actual}`"),
                span,
                format!("provide a value of type `{expected}`"),
            );
        }
    }

    fn error(
        &mut self,
        code: &str,
        message: impl Into<String>,
        span: Span,
        hint: impl Into<String>,
    ) {
        self.diagnostics.push(Diagnostic::new(
            code,
            message,
            self.source,
            span,
            Some(hint.into()),
        ));
    }
}

fn path_params(path: &str) -> HashSet<String> {
    let mut params = HashSet::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('}') else {
            break;
        };
        let name = &after_start[..end];
        if !name.is_empty() {
            params.insert(name.to_string());
        }
        rest = &after_start[end + 1..];
    }
    params
}

fn materialize_inferred(ty: Type) -> Type {
    match ty {
        Type::InferInt => Type::I32,
        Type::InferFloat => Type::F64,
        Type::Result(ok, err) => Type::Result(
            Box::new(materialize_inferred(*ok)),
            Box::new(materialize_inferred(*err)),
        ),
        other => other,
    }
}

fn numeric_result(left: &Type, right: &Type) -> Type {
    match (left, right) {
        (Type::F64, _) | (_, Type::F64) => Type::F64,
        (Type::F32, _) | (_, Type::F32) => Type::F32,
        (Type::I64, _) | (_, Type::I64) => Type::I64,
        (Type::U64, _) | (_, Type::U64) => Type::U64,
        (Type::U32, _) | (_, Type::U32) => Type::U32,
        (Type::I32, _) | (_, Type::I32) => Type::I32,
        (Type::InferFloat, _) | (_, Type::InferFloat) => Type::InferFloat,
        _ => Type::InferInt,
    }
}
