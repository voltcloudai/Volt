use crate::ast::*;
use crate::diagnostics::{Diagnostic, DiagnosticBag, SourceFile, Span};
use crate::route_names::{route_params_type_name, route_query_type_name};
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
    pub errors: HashMap<String, ErrorSig>,
}

#[derive(Debug, Clone)]
pub struct ErrorSig {
    pub variants: HashMap<String, StructSig>,
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

    pub fn insert_error(&mut self, name: impl Into<String>, variants: HashMap<String, StructSig>) {
        self.errors.insert(name.into(), ErrorSig { variants });
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
    errors: HashMap<String, ErrorSig>,
    route_scope: bool,
    require_entrypoint: bool,
}

#[derive(Debug, Clone)]
struct LocalBinding {
    ty: Type,
    declared_ty: Type,
    mutable: bool,
}

type LocalEnv = HashMap<String, LocalBinding>;

impl LocalBinding {
    fn new(ty: Type, mutable: bool) -> Self {
        Self {
            declared_ty: ty.clone(),
            ty,
            mutable,
        }
    }

    fn immutable(ty: Type) -> Self {
        Self::new(ty, false)
    }
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
            errors: external.errors,
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
                Decl::Error(error_decl) => self.check_error_decl(error_decl),
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
                Decl::Error(error_decl) => {
                    if self.errors.contains_key(&error_decl.name)
                        || self.structs.contains_key(&error_decl.name)
                    {
                        self.error(
                            "EERROR001",
                            format!("duplicate error `{}`", error_decl.name),
                            error_decl.span,
                            "error names must be unique",
                        );
                    }
                    self.errors.insert(
                        error_decl.name.clone(),
                        ErrorSig {
                            variants: error_decl
                                .variants
                                .iter()
                                .map(|variant| {
                                    (
                                        variant.name.clone(),
                                        StructSig {
                                            fields: variant
                                                .fields
                                                .iter()
                                                .map(|field| (field.name.clone(), field.ty.clone()))
                                                .collect(),
                                        },
                                    )
                                })
                                .collect(),
                        },
                    );
                }
                Decl::Route(route) => {
                    self.insert_route_generated_types(route);
                }
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
        self.check_route_error_mappings(route);

        if route.handler.is_some() && !route.statements.is_empty() {
            self.error(
                "EHTTP014",
                "route cannot define both an inline body and a handler",
                route.span,
                "move business logic into the handler function or remove the `handler` clause",
            );
        } else if route.handler.is_none() && route.statements.is_empty() {
            self.error(
                "EHTTP015",
                "route must define either an inline body or a handler",
                route.span,
                "add a `{ ... }` route body or `handler myRouteHandler`",
            );
        }

        if let Some(handler) = &route.handler {
            self.check_route_handler(route, handler);
            return;
        }

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
        env.insert(
            "params".to_string(),
            LocalBinding::immutable(Type::Struct(params_type)),
        );
        env.insert(
            "query".to_string(),
            LocalBinding::immutable(Type::Struct(query_type)),
        );
        env.insert("ctx".to_string(), LocalBinding::immutable(Type::Unknown));
        if let Some(body_type) = &route.body_type {
            env.insert(
                "body".to_string(),
                LocalBinding::immutable(body_type.clone()),
            );
        }

        let err_type = route.error_type.clone().unwrap_or(Type::Unknown);
        let return_type = Type::Result(Box::new(route.ok_type.clone()), Box::new(err_type));
        let previous_route_scope = self.route_scope;
        self.route_scope = true;
        for stmt in &route.statements {
            self.check_stmt(stmt, &return_type, &mut env, 0);
        }
        self.route_scope = previous_route_scope;
    }

    fn insert_route_generated_types(&mut self, route: &RouteDecl) {
        if !route.params.is_empty() {
            self.structs.insert(
                route_params_type_name(route),
                StructSig {
                    fields: route
                        .params
                        .iter()
                        .map(|field| (field.name.clone(), field.ty.clone()))
                        .collect(),
                },
            );
        }
        if !route.query.is_empty() {
            self.structs.insert(
                route_query_type_name(route),
                StructSig {
                    fields: route
                        .query
                        .iter()
                        .map(|field| (field.name.clone(), field.ty.clone()))
                        .collect(),
                },
            );
        }
        self.structs.entry("Ctx".to_string()).or_insert(StructSig {
            fields: HashMap::new(),
        });
    }

    fn check_route_handler(&mut self, route: &RouteDecl, handler: &str) {
        let Some(sig) = self.functions.get(handler).cloned() else {
            self.error(
                "EHTTP016",
                format!("unknown route handler `{handler}`"),
                route.span,
                "declare or import the handler function before using it in a route",
            );
            return;
        };

        let expected_return = if let Some(error_type) = &route.error_type {
            Type::Result(
                Box::new(route.ok_type.clone()),
                Box::new(error_type.clone()),
            )
        } else {
            route.ok_type.clone()
        };

        if !expected_return.is_assignable_from(&sig.return_type) {
            self.error(
                "EHTTP017",
                "route handler return type mismatch",
                route.span,
                format!("expected handler `{handler}` to return `{expected_return}`"),
            );
        }

        let expected_params = self.route_handler_params(route);
        if sig.params.len() != expected_params.len()
            || sig
                .params
                .iter()
                .zip(&expected_params)
                .any(|(actual, expected)| !expected.is_assignable_from(actual))
        {
            let expected = expected_params
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            self.error(
                "EHTTP018",
                "route handler parameters do not match route contract",
                route.span,
                format!("expected handler `{handler}` parameters `({expected})`"),
            );
        }
    }

    fn route_handler_params(&self, route: &RouteDecl) -> Vec<Type> {
        let mut params = Vec::new();
        if !route.params.is_empty() {
            params.push(Type::Struct(route_params_type_name(route)));
        }
        if !route.query.is_empty() {
            params.push(Type::Struct(route_query_type_name(route)));
        }
        if let Some(body_type) = &route.body_type {
            params.push(body_type.clone());
        }
        params.push(Type::Struct("Ctx".to_string()));
        params
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

    fn check_route_error_mappings(&mut self, route: &RouteDecl) {
        let Some(error_type) = &route.error_type else {
            return;
        };

        let Type::Struct(error_name) = error_type else {
            self.error(
                "EHTTP006",
                format!("route error type must be an error declaration, found `{error_type}`"),
                route.span,
                "use `errors DomainError { ... }` with an error declaration",
            );
            return;
        };

        let Some(error_sig) = self.errors.get(error_name).cloned() else {
            if self.structs.contains_key(error_name) {
                self.error(
                    "EHTTP007",
                    format!("`{error_name}` is not an error declaration"),
                    route.span,
                    "declare it with `error`, not `type`",
                );
            } else {
                self.error(
                    "EHTTP008",
                    format!("unknown route error type `{error_name}`"),
                    route.span,
                    "import or declare the error type before using it in route errors",
                );
            }
            return;
        };

        for mapping in &route.errors {
            if !error_sig.variants.contains_key(&mapping.name) {
                self.error(
                    "EHTTP009",
                    format!("error `{error_name}` has no variant `{}`", mapping.name),
                    mapping.span,
                    "map a declared error variant",
                );
            }
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

    fn check_error_decl(&mut self, error_decl: &ErrorDecl) {
        let mut seen = HashSet::new();
        for variant in &error_decl.variants {
            if !seen.insert(variant.name.clone()) {
                self.error(
                    "EERROR002",
                    format!("duplicate error variant `{}`", variant.name),
                    variant.span,
                    "variant names must be unique within an error declaration",
                );
            }

            let mut seen_fields = HashSet::new();
            for field in &variant.fields {
                if !seen_fields.insert(field.name.clone()) {
                    self.error(
                        "EERROR003",
                        format!("duplicate field `{}`", field.name),
                        field.span,
                        "field names must be unique within an error variant",
                    );
                }
                self.check_type_exists(&field.ty, field.span);
            }
        }
    }

    fn check_function(&mut self, function: &FunctionDecl) {
        self.check_type_exists(&function.return_type, function.span);

        let mut env = HashMap::new();
        for param in &function.params {
            self.check_type_exists(&param.ty, param.span);
            if env
                .insert(
                    param.name.clone(),
                    LocalBinding::immutable(param.ty.clone()),
                )
                .is_some()
            {
                self.error(
                    "E104",
                    format!("duplicate parameter `{}`", param.name),
                    param.span,
                    "parameter names must be unique",
                );
            }
        }

        for stmt in &function.body {
            self.check_stmt(stmt, &function.return_type, &mut env, 0);
        }
    }

    fn check_stmt(
        &mut self,
        stmt: &Stmt,
        return_type: &Type,
        env: &mut LocalEnv,
        loop_depth: usize,
    ) {
        match stmt {
            Stmt::Var {
                mutable,
                name,
                annotation,
                expr,
                span,
            } => {
                if let Some(annotation) = annotation {
                    self.check_type_exists(annotation, *span);
                    let actual = self.check_expr(expr, env, Some(annotation));
                    self.expect_type(annotation, &actual, expr.span());
                    env.insert(
                        name.clone(),
                        LocalBinding::new(annotation.clone(), *mutable),
                    );
                } else {
                    let actual = self.check_expr(expr, env, None);
                    let ty = materialize_inferred(actual);
                    env.insert(name.clone(), LocalBinding::new(ty, *mutable));
                }
            }
            Stmt::Assign { name, expr, span } => {
                let Some(binding) = env.get(name).cloned() else {
                    self.error(
                        "E130",
                        format!("cannot assign to unknown variable `{name}`"),
                        *span,
                        "declare the variable with `let` before assigning to it",
                    );
                    self.check_expr(expr, env, None);
                    return;
                };

                if !binding.mutable {
                    self.error(
                        "E131",
                        format!("cannot assign to immutable variable `{name}`"),
                        *span,
                        "use `let` for mutable local variables",
                    );
                }

                let actual = self.check_expr(expr, env, Some(&binding.declared_ty));
                if !binding.declared_ty.is_assignable_from(&actual) {
                    self.error(
                        "E132",
                        format!(
                            "assignment type mismatch: expected `{}`, found `{actual}`",
                            binding.declared_ty
                        ),
                        expr.span(),
                        format!("assign a value of type `{}`", binding.declared_ty),
                    );
                }

                env.insert(
                    name.clone(),
                    LocalBinding::new(binding.declared_ty, binding.mutable),
                );
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
                let condition_info = self.check_condition(condition, env);

                let mut then_env = env.clone();
                if let Some((name, narrowed)) = &condition_info.positive_narrow {
                    narrow_local(&mut then_env, name, narrowed.clone());
                }
                for stmt in then_body {
                    self.check_stmt(stmt, return_type, &mut then_env, loop_depth);
                }
                let mut else_env = env.clone();
                for stmt in else_body {
                    self.check_stmt(stmt, return_type, &mut else_env, loop_depth);
                }

                if else_body.is_empty() && always_returns(then_body) {
                    if let Some((name, narrowed)) = condition_info.negative_guard_narrow {
                        narrow_local(env, &name, narrowed);
                    }
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.check_condition(condition, env);
                let mut body_env = env.clone();
                for stmt in body {
                    self.check_stmt(stmt, return_type, &mut body_env, loop_depth + 1);
                }
            }
            Stmt::ForIn {
                item,
                iterable,
                body,
                ..
            } => {
                let iterable_ty = self.check_expr(iterable, env, None);
                let item_ty = match iterable_ty {
                    Type::Array(inner) => *inner,
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.error(
                            "E152",
                            format!("for-in requires Array<T>, found `{other}`"),
                            iterable.span(),
                            "iterate over an `Array<T>` value",
                        );
                        Type::Unknown
                    }
                };

                let mut body_env = env.clone();
                body_env.insert(item.clone(), LocalBinding::immutable(item_ty));
                for stmt in body {
                    self.check_stmt(stmt, return_type, &mut body_env, loop_depth + 1);
                }
            }
            Stmt::Break { span } => {
                if loop_depth == 0 {
                    self.error(
                        "E150",
                        "`break` can only be used inside a loop",
                        *span,
                        "move `break` into a `while` or `for-in` loop",
                    );
                }
            }
            Stmt::Continue { span } => {
                if loop_depth == 0 {
                    self.error(
                        "E151",
                        "`continue` can only be used inside a loop",
                        *span,
                        "move `continue` into a `while` or `for-in` loop",
                    );
                }
            }
            Stmt::Switch {
                expr,
                cases,
                default,
                ..
            } => {
                let switch_ty = self.check_expr(expr, env, None);
                for case in cases {
                    let case_ty = self.check_expr(&case.value, env, Some(&switch_ty));
                    if !switch_ty.is_assignable_from(&case_ty)
                        && !case_ty.is_assignable_from(&switch_ty)
                    {
                        self.error(
                            "E154",
                            format!("switch case type mismatch: cannot compare `{switch_ty}` with `{case_ty}`"),
                            case.value.span(),
                            "use case values with the same type as the switch expression",
                        );
                    }
                    let mut case_env = env.clone();
                    for stmt in &case.body {
                        self.check_stmt(stmt, return_type, &mut case_env, loop_depth);
                    }
                }
                let mut default_env = env.clone();
                for stmt in default {
                    self.check_stmt(stmt, return_type, &mut default_env, loop_depth);
                }
            }
            Stmt::Expr { expr, .. } => {
                self.check_expr(expr, env, None);
            }
        }
    }

    fn check_condition(&mut self, condition: &Expr, env: &LocalEnv) -> ConditionInfo {
        match condition {
            Expr::Unary {
                op: UnaryOp::Not,
                expr,
                ..
            } => {
                let inner_ty = self.check_expr(expr, env, None);
                match inner_ty {
                    Type::Bool => ConditionInfo::default(),
                    Type::Option(inner) => {
                        let mut info = ConditionInfo::default();
                        if let Expr::Var { name, .. } = expr.as_ref() {
                            info.negative_guard_narrow = Some((name.clone(), *inner));
                        }
                        info
                    }
                    Type::Unknown => ConditionInfo::default(),
                    other => {
                        self.error(
                            "E126",
                            format!("`!` requires bool or Option<T>, found `{other}`"),
                            condition.span(),
                            "use `!` only with booleans or Option values",
                        );
                        ConditionInfo::default()
                    }
                }
            }
            Expr::Var { name, .. } => {
                let actual = self.check_expr(condition, env, None);
                match actual {
                    Type::Bool => ConditionInfo::default(),
                    Type::Option(inner) => ConditionInfo {
                        positive_narrow: Some((name.clone(), *inner)),
                        negative_guard_narrow: None,
                    },
                    Type::Unknown => ConditionInfo::default(),
                    other => {
                        self.error(
                            "E125",
                            format!("if condition must be bool or Option<T>, found `{other}`"),
                            condition.span(),
                            "use explicit comparisons for strings, numbers, and objects",
                        );
                        ConditionInfo::default()
                    }
                }
            }
            _ => {
                let actual = self.check_expr(condition, env, None);
                match actual {
                    Type::Bool | Type::Unknown => ConditionInfo::default(),
                    Type::Option(_) => ConditionInfo::default(),
                    other => {
                        self.error(
                            "E125",
                            format!("if condition must be bool or Option<T>, found `{other}`"),
                            condition.span(),
                            "use explicit comparisons for strings, numbers, and objects",
                        );
                        ConditionInfo::default()
                    }
                }
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr, env: &LocalEnv, expected: Option<&Type>) -> Type {
        match expr {
            Expr::Int { .. } => Type::InferInt,
            Expr::Float { .. } => Type::InferFloat,
            Expr::String { .. } => Type::String,
            Expr::Bool { .. } => Type::Bool,
            Expr::Var { name, span } if name == "none" => {
                if expected.is_some_and(|ty| !matches!(ty, Type::Option(_))) {
                    self.error(
                        "EOPTION001",
                        "`none` can only be used where Option<T> is expected",
                        *span,
                        "return or assign `none` only in an Option<T> context",
                    );
                }
                Type::None
            }
            Expr::Var { name, span } => env
                .get(name)
                .map(|binding| binding.ty.clone())
                .unwrap_or_else(|| {
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
            Expr::MethodCall {
                object,
                method,
                args,
                span,
            } => self.check_method_call(object, method, args, *span, env),
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
            Expr::ArrayLiteral { elements, span } => {
                self.check_array_literal(elements, *span, env, expected)
            }
            Expr::StructLiteral { name, fields, span } => {
                self.check_struct_literal(name, fields, *span, env)
            }
            Expr::ErrorVariantLiteral {
                error,
                variant,
                fields,
                span,
            } => self.check_error_variant_literal(error, variant, fields, *span, env),
            Expr::FieldAccess {
                object,
                field,
                span,
            } => {
                let object_ty = self.check_expr(object, env, None);
                match object_ty {
                    Type::Array(_) if field == "length" => Type::U64,
                    Type::Array(_) => {
                        self.error(
                            "E109",
                            format!("cannot access field `{field}` on array"),
                            *span,
                            "arrays currently support only the `.length` field",
                        );
                        Type::Unknown
                    }
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
            Expr::Index {
                target,
                index,
                span,
            } => {
                let target_ty = self.check_expr(target, env, None);
                let index_ty = self.check_expr(index, env, None);
                if !matches!(index_ty, Type::Unknown) && !index_ty.is_integer() {
                    self.error(
                        "E161",
                        format!("array index must be an integer, found `{index_ty}`"),
                        index.span(),
                        "use an i32, i64, u32, u64, or inferred integer index",
                    );
                }
                match target_ty {
                    Type::Array(inner) => *inner,
                    Type::Unknown => Type::Unknown,
                    other => {
                        self.error(
                            "E160",
                            format!("cannot index non-array type `{other}`"),
                            *span,
                            "index only values of type `Array<T>`",
                        );
                        Type::Unknown
                    }
                }
            }
            Expr::Unary { op, expr, span } => match op {
                UnaryOp::Not => {
                    let actual = self.check_expr(expr, env, None);
                    match actual {
                        Type::Bool | Type::Option(_) | Type::Unknown => Type::Bool,
                        other => {
                            self.error(
                                "E126",
                                format!("`!` requires bool or Option<T>, found `{other}`"),
                                *span,
                                "use `!` only with booleans or Option values",
                            );
                            Type::Bool
                        }
                    }
                }
            },
        }
    }

    fn check_method_call(
        &mut self,
        object: &Expr,
        method: &str,
        args: &[Expr],
        span: Span,
        env: &LocalEnv,
    ) -> Type {
        let object_ty = self.check_expr(object, env, None);
        match object_ty {
            Type::Array(inner) if method == "push" => {
                if args.len() != 1 {
                    self.error(
                        "E163",
                        format!("array push expects one argument, found {}", args.len()),
                        span,
                        "call `array.push(value)` with exactly one value",
                    );
                    for arg in args {
                        self.check_expr(arg, env, None);
                    }
                    return Type::Void;
                }

                if !mutable_local_target(object, env) {
                    self.error(
                        "E165",
                        "cannot mutate immutable array",
                        object.span(),
                        "call `push` only on a mutable `let` array binding",
                    );
                }

                let actual = self.check_expr(&args[0], env, Some(&inner));
                if !inner.is_assignable_from(&actual) {
                    self.error(
                        "E164",
                        format!(
                            "array push value type mismatch: expected `{inner}`, found `{actual}`"
                        ),
                        args[0].span(),
                        format!("push a value of type `{inner}`"),
                    );
                }

                Type::Void
            }
            Type::Array(_) => {
                self.error(
                    "E162",
                    format!("unknown method `{method}` for array"),
                    span,
                    "arrays currently support only `push(value)`",
                );
                for arg in args {
                    self.check_expr(arg, env, None);
                }
                Type::Unknown
            }
            Type::Unknown => {
                for arg in args {
                    self.check_expr(arg, env, None);
                }
                Type::Unknown
            }
            other => {
                self.error(
                    "E162",
                    format!("unknown method `{method}` for type `{other}`"),
                    span,
                    "method calls are currently supported only for arrays",
                );
                for arg in args {
                    self.check_expr(arg, env, None);
                }
                Type::Unknown
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
            BinaryOp::And | BinaryOp::Or => {
                if matches!(left, Type::Bool | Type::Unknown)
                    && matches!(right, Type::Bool | Type::Unknown)
                {
                    Type::Bool
                } else {
                    self.error(
                        "E133",
                        format!(
                            "boolean operator requires bool operands, found `{left}` and `{right}`"
                        ),
                        span,
                        "use `&&` and `||` only with bool values",
                    );
                    Type::Bool
                }
            }
        }
    }

    fn check_array_literal(
        &mut self,
        elements: &[Expr],
        span: Span,
        env: &LocalEnv,
        expected: Option<&Type>,
    ) -> Type {
        if let Some(expected) = expected {
            let Type::Array(inner) = expected else {
                self.error(
                    "E142",
                    format!("expected array type, found `{expected}`"),
                    span,
                    "use an `Array<T>` contextual type for array literals",
                );
                for element in elements {
                    self.check_expr(element, env, None);
                }
                return Type::Unknown;
            };

            for element in elements {
                let actual = self.check_expr(element, env, Some(inner));
                self.expect_type(inner, &actual, element.span());
            }
            return Type::Array(Box::new((**inner).clone()));
        }

        let Some((first, rest)) = elements.split_first() else {
            self.error(
                "E140",
                "cannot infer type of empty array",
                span,
                "add a contextual type, for example `const ids: Array<u64> = []`",
            );
            return Type::Array(Box::new(Type::Unknown));
        };

        let first_ty = materialize_inferred(self.check_expr(first, env, None));
        if matches!(first_ty, Type::None | Type::Unknown) {
            self.error(
                "E140",
                "cannot infer array element type",
                span,
                "add an `Array<T>` annotation so contextual typing can be used",
            );
            return Type::Array(Box::new(Type::Unknown));
        }

        for element in rest {
            let actual = self.check_expr(element, env, Some(&first_ty));
            if !first_ty.is_assignable_from(&actual) {
                self.error(
                    "E141",
                    format!("array elements must have the same type, expected `{first_ty}`"),
                    element.span(),
                    "use values with one shared element type",
                );
            }
        }

        Type::Array(Box::new(first_ty))
    }

    fn check_call(
        &mut self,
        callee: &str,
        args: &[Expr],
        span: Span,
        env: &LocalEnv,
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
                if self.structs.contains_key(callee) {
                    if args.len() != 1 {
                        self.error(
                            "E127",
                            format!("type constructor `{callee}` expects one object argument"),
                            span,
                            "construct values with `Type({ field: value })`",
                        );
                        return Type::Struct(callee.to_string());
                    }
                    let Some(fields) = fields_from_object_literal(&args[0]) else {
                        self.error(
                            "E128",
                            format!("type constructor `{callee}` expects an object literal"),
                            args[0].span(),
                            "construct values with `Type({ field: value })`",
                        );
                        self.check_expr(&args[0], env, None);
                        return Type::Struct(callee.to_string());
                    };
                    return self.check_struct_literal(callee, &fields, span, env);
                }

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
        env: &LocalEnv,
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

    fn check_error_variant_literal(
        &mut self,
        error: &str,
        variant: &str,
        fields: &[FieldValue],
        span: Span,
        env: &LocalEnv,
    ) -> Type {
        let Some(error_sig) = self.errors.get(error).cloned() else {
            if self.structs.contains_key(error) {
                self.error(
                    "EERROR004",
                    format!("`{error}` is a type, not an error declaration"),
                    span,
                    "declare domain errors with `error`",
                );
            } else {
                self.error(
                    "EERROR005",
                    format!("unknown error `{error}`"),
                    span,
                    "declare or import the error before constructing its variants",
                );
            }
            for field in fields {
                self.check_expr(&field.expr, env, None);
            }
            return Type::Unknown;
        };

        let Some(variant_sig) = error_sig.variants.get(variant) else {
            self.error(
                "EERROR006",
                format!("error `{error}` has no variant `{variant}`"),
                span,
                "use a declared error variant",
            );
            for field in fields {
                self.check_expr(&field.expr, env, None);
            }
            return Type::Struct(error.to_string());
        };

        self.check_field_values(
            &format!("{error}.{variant}"),
            &variant_sig.fields,
            fields,
            span,
            env,
        );
        Type::Struct(error.to_string())
    }

    fn check_field_values(
        &mut self,
        owner: &str,
        expected_fields: &HashMap<String, Type>,
        fields: &[FieldValue],
        span: Span,
        env: &LocalEnv,
    ) {
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

            let Some(expected) = expected_fields.get(&field.name) else {
                self.error(
                    "E120",
                    format!("`{owner}` has no field `{}`", field.name),
                    field.span,
                    "use a declared field",
                );
                self.check_expr(&field.expr, env, None);
                continue;
            };

            let actual = self.check_expr(&field.expr, env, Some(expected));
            self.expect_type(expected, &actual, field.expr.span());
        }

        for required in expected_fields.keys() {
            if !seen.contains(required) {
                self.error(
                    "E121",
                    format!("missing field `{required}` for `{owner}`"),
                    span,
                    "initialize every required field",
                );
            }
        }
    }

    fn check_type_exists(&mut self, ty: &Type, span: Span) {
        match ty {
            Type::Struct(name)
                if !self.structs.contains_key(name) && !self.errors.contains_key(name) =>
            {
                self.error(
                    "E122",
                    format!("unknown type `{name}`"),
                    span,
                    "declare this type before using it",
                )
            }
            Type::Option(inner) => self.check_type_exists(inner, span),
            Type::Result(ok, err) => {
                self.check_type_exists(ok, span);
                self.check_type_exists(err, span);
            }
            Type::Array(inner) => self.check_type_exists(inner, span),
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
        Type::Option(inner) => Type::Option(Box::new(materialize_inferred(*inner))),
        Type::Result(ok, err) => Type::Result(
            Box::new(materialize_inferred(*ok)),
            Box::new(materialize_inferred(*err)),
        ),
        Type::Array(inner) => Type::Array(Box::new(materialize_inferred(*inner))),
        other => other,
    }
}

fn mutable_local_target(expr: &Expr, env: &LocalEnv) -> bool {
    let Expr::Var { name, .. } = expr else {
        return false;
    };
    env.get(name)
        .map(|binding| binding.mutable)
        .unwrap_or(false)
}

#[derive(Debug, Default)]
struct ConditionInfo {
    positive_narrow: Option<(String, Type)>,
    negative_guard_narrow: Option<(String, Type)>,
}

fn always_returns(statements: &[Stmt]) -> bool {
    statements.iter().any(|stmt| match stmt {
        Stmt::Return { .. } => true,
        Stmt::If {
            then_body,
            else_body,
            ..
        } if !else_body.is_empty() => always_returns(then_body) && always_returns(else_body),
        Stmt::While {
            condition, body, ..
        } if matches!(condition, Expr::Bool { value: true, .. }) => always_returns(body),
        Stmt::Switch { cases, default, .. } if !default.is_empty() => {
            cases.iter().all(|case| always_returns(&case.body)) && always_returns(default)
        }
        _ => false,
    })
}

fn narrow_local(env: &mut LocalEnv, name: &str, ty: Type) {
    if let Some(binding) = env.get_mut(name) {
        binding.ty = ty;
    }
}

fn fields_from_object_literal(expr: &Expr) -> Option<Vec<FieldValue>> {
    let Expr::ObjectLiteral { fields, .. } = expr else {
        return None;
    };

    let mut values = Vec::new();
    for field in fields {
        match field {
            ObjectField::Named { name, expr, span } => values.push(FieldValue {
                name: name.clone(),
                expr: expr.clone(),
                span: *span,
            }),
            ObjectField::Spread { .. } => return None,
        }
    }
    Some(values)
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
