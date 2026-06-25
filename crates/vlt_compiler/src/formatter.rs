use crate::ast::*;
use crate::types::Type;

pub fn format_program(program: &Program) -> String {
    let mut out = String::new();
    for (idx, declaration) in program.declarations.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        match declaration {
            Decl::Function(function) => format_function(&mut out, function),
            Decl::Type(type_decl) => format_type_decl(&mut out, type_decl),
            Decl::Error(error_decl) => format_error_decl(&mut out, error_decl),
            Decl::Route(route) => format_route(&mut out, route),
            Decl::Import(import) => format_import(&mut out, import),
        }
    }
    out
}

fn format_import(out: &mut String, import: &ImportDecl) {
    let items = import
        .items
        .iter()
        .map(|item| item.name.clone())
        .collect::<Vec<_>>()
        .join(", ");

    out.push_str(&format!(
        "import {{ {} }} from {:?}\n",
        items, import.module
    ));
}

fn format_type_decl(out: &mut String, type_decl: &TypeDecl) {
    if type_decl.exported {
        out.push_str("export ");
    }
    out.push_str(&format!("type {} = {{\n", type_decl.name));

    for field in &type_decl.fields {
        out.push_str(&format!("  {}: {}\n", field.name, format_type(&field.ty)));
    }

    out.push_str("}\n");
}

fn format_error_decl(out: &mut String, error_decl: &ErrorDecl) {
    if error_decl.exported {
        out.push_str("export ");
    }
    out.push_str(&format!("error {} {{\n", error_decl.name));
    for variant in &error_decl.variants {
        let fields = variant
            .fields
            .iter()
            .map(|field| format!("{}: {}", field.name, format_type(&field.ty)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("  {} {{ {} }}\n", variant.name, fields));
    }
    out.push_str("}\n");
}

fn format_function(out: &mut String, function: &FunctionDecl) {
    if function.exported {
        out.push_str("export ");
    }

    let params = function
        .params
        .iter()
        .map(|param| format!("{}: {}", param.name, format_type(&param.ty)))
        .collect::<Vec<_>>()
        .join(", ");

    out.push_str(&format!(
        "function {}({}): {} {{\n",
        function.name,
        params,
        format_type(&function.return_type)
    ));

    for stmt in &function.body {
        format_stmt(out, stmt, 1);
    }

    out.push_str("}\n");
}

fn format_route(out: &mut String, route: &RouteDecl) {
    if route.exported {
        out.push_str("export ");
    }

    out.push_str(&format!(
        "route {} {:?}\n",
        format_method(route.method),
        route.path
    ));
    if !route.params.is_empty() {
        format_route_fields(out, "params", &route.params);
    }
    if !route.query.is_empty() {
        format_route_fields(out, "query", &route.query);
    }
    if let Some(body_type) = &route.body_type {
        out.push_str(&format!("  body {}\n", format_type(body_type)));
    }
    out.push_str(&format!(
        "  ok {} {}\n",
        route.ok_status,
        format_type(&route.ok_type)
    ));
    if !route.errors.is_empty() {
        if let Some(error_type) = &route.error_type {
            out.push_str(&format!("  errors {} {{\n", format_type(error_type)));
        } else {
            out.push_str("  errors {\n");
        }
        for error in &route.errors {
            out.push_str(&format!("    {} {}\n", error.name, error.status));
        }
        out.push_str("  }\n");
    }
    if !route.effects.is_empty() {
        out.push_str(&format!("  effects [{}]\n", route.effects.join(", ")));
    }
    out.push_str("{\n");
    for stmt in &route.statements {
        format_stmt(out, stmt, 1);
    }
    out.push_str("}\n");
}

fn format_route_fields(out: &mut String, label: &str, fields: &[RouteField]) {
    out.push_str(&format!("  {label} {{ "));
    out.push_str(
        &fields
            .iter()
            .map(|field| format!("{}: {}", field.name, format_type(&field.ty)))
            .collect::<Vec<_>>()
            .join(", "),
    );
    out.push_str(" }\n");
}

fn format_stmt(out: &mut String, stmt: &Stmt, indent: usize) {
    let pad = "  ".repeat(indent);
    match stmt {
        Stmt::Var {
            mutable,
            name,
            annotation,
            expr,
            ..
        } => {
            let keyword = if *mutable { "let" } else { "const" };
            let annotation = annotation
                .as_ref()
                .map(|ty| format!(": {}", format_type(ty)))
                .unwrap_or_default();
            out.push_str(&format!(
                "{pad}{keyword} {name}{annotation} = {}\n",
                format_expr(expr)
            ));
        }
        Stmt::Return { expr, .. } => {
            out.push_str(&format!("{pad}return {}\n", format_expr(expr)));
        }
        Stmt::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            out.push_str(&format!("{pad}if ({}) {{\n", format_expr(condition)));
            for stmt in then_body {
                format_stmt(out, stmt, indent + 1);
            }
            if else_body.is_empty() {
                out.push_str(&format!("{pad}}}\n"));
            } else {
                out.push_str(&format!("{pad}}} else {{\n"));
                for stmt in else_body {
                    format_stmt(out, stmt, indent + 1);
                }
                out.push_str(&format!("{pad}}}\n"));
            }
        }
        Stmt::Expr { expr, .. } => {
            out.push_str(&format!("{pad}{}\n", format_expr(expr)));
        }
    }
}

fn format_expr(expr: &Expr) -> String {
    match expr {
        Expr::Int { value, .. } => value.to_string(),
        Expr::Float { value, .. } => value.to_string(),
        Expr::String { value, .. } => format!("{value:?}"),
        Expr::Bool { value, .. } => value.to_string(),
        Expr::Var { name, .. } => name.clone(),
        Expr::Binary {
            left, op, right, ..
        } => format!(
            "{} {} {}",
            format_expr(left),
            format_op(*op),
            format_expr(right)
        ),
        Expr::Call { callee, args, .. } => format!(
            "{}({})",
            callee,
            args.iter().map(format_expr).collect::<Vec<_>>().join(", ")
        ),
        Expr::Try { expr, .. } => format!("try {}", format_expr(expr)),
        Expr::Unary { op, expr, .. } => match op {
            UnaryOp::Not => format!("!{}", format_expr(expr)),
        },
        Expr::ObjectLiteral { fields, .. } => {
            let body = fields
                .iter()
                .map(|field| match field {
                    ObjectField::Named { name, expr, .. } => {
                        format!("{}: {}", name, format_expr(expr))
                    }
                    ObjectField::Spread { expr, .. } => format!("...{}", format_expr(expr)),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {body} }}")
        }
        Expr::StructLiteral { name, fields, .. } => {
            let mut out = format!("{name} {{ ");
            out.push_str(
                &fields
                    .iter()
                    .map(|field| format!("{}: {}", field.name, format_expr(&field.expr)))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            out.push_str(" }");
            out
        }
        Expr::ErrorVariantLiteral {
            error,
            variant,
            fields,
            ..
        } => {
            let body = fields
                .iter()
                .map(|field| format!("{}: {}", field.name, format_expr(&field.expr)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{error}.{variant}({{ {body} }})")
        }
        Expr::FieldAccess { object, field, .. } => format!("{}.{}", format_expr(object), field),
    }
}

fn format_method(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "get",
        HttpMethod::Post => "post",
        HttpMethod::Put => "put",
        HttpMethod::Patch => "patch",
        HttpMethod::Delete => "delete",
    }
}

fn format_op(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Eq => "===",
        BinaryOp::NotEq => "!==",
        BinaryOp::Lt => "<",
        BinaryOp::Gt => ">",
        BinaryOp::LtEq => "<=",
        BinaryOp::GtEq => ">=",
    }
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::Option(inner) => format!("Option<{}>", format_type(inner)),
        Type::Result(ok, err) => format!("Result<{}, {}>", format_type(ok), format_type(err)),
        other => other.to_string(),
    }
}
