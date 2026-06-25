use crate::ast::*;
use crate::diagnostics::{Diagnostic, DiagnosticBag, SourceFile, Span};
use crate::lexer::{lex, Token, TokenKind};
use crate::types::Type;

pub fn parse_source(source: &SourceFile) -> Result<Program, DiagnosticBag> {
    let tokens = lex(source)?;
    Parser {
        source,
        tokens,
        pos: 0,
        diagnostics: DiagnosticBag::new(),
        suppress_struct_literal: false,
    }
    .parse_program()
}

struct Parser<'a> {
    source: &'a SourceFile,
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: DiagnosticBag,
    suppress_struct_literal: bool,
}

impl Parser<'_> {
    fn parse_program(mut self) -> Result<Program, DiagnosticBag> {
        let mut declarations = Vec::new();

        while !self.at(TokenKindName::Eof) {
            match self.peek_kind() {
                TokenKind::Import => declarations.push(Decl::Import(self.parse_import_decl())),
                TokenKind::Export => declarations.push(self.parse_export_decl()),
                TokenKind::Function => {
                    declarations.push(Decl::Function(self.parse_function(false)))
                }
                TokenKind::Type => declarations.push(Decl::Type(self.parse_type_decl(false))),
                TokenKind::Error => declarations.push(Decl::Error(self.parse_error_decl(false))),
                TokenKind::Route => declarations.push(Decl::Route(self.parse_route_decl(false))),
                _ => {
                    let token = self.peek().clone();
                    self.error(
                        "E010",
                        "expected top-level declaration",
                        token.span,
                        "start with `import`, `export`, `function`, `type`, `error`, or `route`",
                    );
                    self.advance();
                }
            }
        }

        if self.diagnostics.is_empty() {
            Ok(Program { declarations })
        } else {
            Err(self.diagnostics)
        }
    }

    fn parse_export_decl(&mut self) -> Decl {
        self.expect(TokenKindName::Export, "expected `export`");

        match self.peek_kind() {
            TokenKind::Function => Decl::Function(self.parse_function(true)),
            TokenKind::Type => Decl::Type(self.parse_type_decl(true)),
            TokenKind::Error => Decl::Error(self.parse_error_decl(true)),
            TokenKind::Route => Decl::Route(self.parse_route_decl(true)),
            _ => {
                let token = self.peek().clone();
                self.error(
                    "EEXPORT001",
                    "expected declaration after `export`",
                    token.span,
                    "export a `function`, `type`, `error`, or `route`",
                );
                self.advance();

                Decl::Type(TypeDecl {
                    exported: true,
                    name: "<error>".to_string(),
                    fields: Vec::new(),
                    span: token.span,
                })
            }
        }
    }

    fn parse_import_decl(&mut self) -> ImportDecl {
        let start = self
            .expect(TokenKindName::Import, "expected `import`")
            .start;

        self.expect(TokenKindName::LBrace, "expected `{` after `import`");

        let mut items = Vec::new();
        while !self.at(TokenKindName::RBrace) && !self.at(TokenKindName::Eof) {
            let (name, span) = self.expect_ident("expected imported symbol name");
            items.push(ImportItem { name, span });

            if !self.eat(TokenKindName::Comma) {
                break;
            }
        }

        self.expect(TokenKindName::RBrace, "expected `}` after import list");
        self.expect(TokenKindName::From, "expected `from` after import list");

        let module_token = self.advance();
        let module = if let TokenKind::String(value) = module_token.kind {
            value
        } else {
            self.error(
                "EIMPORT001",
                "expected module path string after `from`",
                module_token.span,
                "write `from \"./module\"`",
            );
            String::new()
        };

        ImportDecl {
            items,
            module,
            span: Span::new(start, module_token.span.end),
        }
    }

    fn parse_route_decl(&mut self, exported: bool) -> RouteDecl {
        let start = self.expect(TokenKindName::Route, "expected `route`").start;
        let (method, method_span) = self.parse_http_method();
        let path_token = self.advance();
        let path = if let TokenKind::String(value) = path_token.kind {
            value
        } else {
            self.error(
                "EHTTP010",
                "expected route path string",
                path_token.span,
                "write a string path like `\"/users/{id}\"`",
            );
            "/".to_string()
        };

        let mut params = Vec::new();
        let mut query = Vec::new();
        let mut body_type = None;
        let mut ok_status = None;
        let mut ok_type = None;
        let mut error_type = None;
        let mut errors = Vec::new();
        let mut effects = Vec::new();
        let mut handler = None;

        while !self.at(TokenKindName::LBrace)
            && !self.at(TokenKindName::Eof)
            && !self.at_route_decl_boundary()
        {
            if self.eat_keyword("params") {
                params = self.parse_route_fields("params");
            } else if self.eat_keyword("query") {
                query = self.parse_route_fields("query");
            } else if self.eat_keyword("body") {
                body_type = Some(self.parse_type());
            } else if self.eat_keyword("ok") {
                let (status, _) = self.parse_status_code("expected HTTP success status after `ok`");
                ok_status = Some(status);
                ok_type = Some(self.parse_type());
            } else if self.eat_keyword("errors") {
                if self.at(TokenKindName::LBrace) {
                    errors = self.parse_route_errors();
                } else {
                    error_type = Some(self.parse_type());
                    errors = self.parse_route_errors();
                }
            } else if self.eat_keyword("effects") {
                effects = self.parse_effects();
            } else if self.eat_keyword("handler") {
                if self.at(TokenKindName::LBrace)
                    || self.at(TokenKindName::Eof)
                    || self.at_route_decl_boundary()
                {
                    let span = self.peek().span;
                    self.error(
                        "EHTTP011",
                        "expected route handler name",
                        span,
                        "write `handler myRouteHandler` with an identifier",
                    );
                    break;
                }
                let token = self.advance();
                if let TokenKind::Ident(name) = token.kind {
                    handler = Some(name);
                } else {
                    self.error(
                        "EHTTP011",
                        "expected route handler name",
                        token.span,
                        "write `handler myRouteHandler` with an identifier",
                    );
                }
                if !self.at(TokenKindName::LBrace) && !self.at_route_clause_start() {
                    break;
                }
            } else {
                let token = self.peek().clone();
                self.error(
                    "EHTTP011",
                    "expected route clause",
                    token.span,
                    "use `params`, `query`, `body`, `ok`, `errors`, `effects`, or `handler`",
                );
                self.advance();
            }
        }

        if ok_status.is_none() || ok_type.is_none() {
            self.error(
                "EHTTP012",
                "route is missing required `ok <status> <Type>` clause",
                method_span,
                "add an `ok 200 ResponseType` clause before the route body",
            );
        }

        let has_block = self.at(TokenKindName::LBrace);
        if handler.is_some() && has_block {
            self.error(
                "EHTTP014",
                "route cannot define both an inline body and a handler",
                self.peek().span,
                "move business logic into the handler function or remove the `handler` clause",
            );
        } else if handler.is_none() && !has_block {
            self.error(
                "EHTTP015",
                "route must define either an inline body or a handler",
                method_span,
                "add a `{ ... }` route body or `handler myRouteHandler`",
            );
        }

        let statements = if has_block {
            self.parse_block()
        } else {
            Vec::new()
        };
        let end = statements
            .last()
            .map(stmt_span)
            .unwrap_or(method_span)
            .end
            .max(self.previous_span().end);

        RouteDecl {
            exported,
            method,
            path,
            params,
            query,
            body_type,
            ok_status: ok_status.unwrap_or(200),
            ok_type: ok_type.unwrap_or(Type::Unknown),
            error_type,
            errors,
            effects,
            handler,
            statements,
            span: Span::new(start, end),
        }
    }

    fn at_route_decl_boundary(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Route
                | TokenKind::Function
                | TokenKind::Type
                | TokenKind::Error
                | TokenKind::Import
                | TokenKind::Export
        )
    }

    fn at_route_clause_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Ident(name)
                if matches!(
                    name.as_str(),
                    "params" | "query" | "body" | "ok" | "errors" | "effects" | "handler"
                )
        )
    }

    fn parse_http_method(&mut self) -> (HttpMethod, Span) {
        let token = self.advance();
        let span = token.span;
        let TokenKind::Ident(name) = token.kind else {
            self.error(
                "EHTTP013",
                "expected lowercase HTTP method",
                span,
                "use one of `get`, `post`, `put`, `patch`, or `delete`",
            );
            return (HttpMethod::Get, span);
        };

        let method = match name.as_str() {
            "get" => HttpMethod::Get,
            "post" => HttpMethod::Post,
            "put" => HttpMethod::Put,
            "patch" => HttpMethod::Patch,
            "delete" => HttpMethod::Delete,
            _ => {
                self.error(
                    "EHTTP013",
                    format!("unsupported HTTP method `{name}`"),
                    span,
                    "use one of `get`, `post`, `put`, `patch`, or `delete`",
                );
                HttpMethod::Get
            }
        };
        (method, span)
    }

    fn parse_route_fields(&mut self, name: &str) -> Vec<RouteField> {
        self.expect(
            TokenKindName::LBrace,
            &format!("expected `{{` after `{name}`"),
        );
        let mut fields = Vec::new();
        while !self.at(TokenKindName::RBrace) && !self.at(TokenKindName::Eof) {
            let (field_name, span) = self.expect_ident("expected route field name");
            self.expect(TokenKindName::Colon, "expected `:` after route field name");
            let ty = self.parse_type();
            fields.push(RouteField {
                name: field_name,
                ty,
                span,
            });
            self.eat(TokenKindName::Comma);
        }
        self.expect(TokenKindName::RBrace, "expected `}` after route fields");
        fields
    }

    fn parse_route_errors(&mut self) -> Vec<RouteErrorMapping> {
        self.expect(TokenKindName::LBrace, "expected `{` after `errors`");
        let mut errors = Vec::new();
        while !self.at(TokenKindName::RBrace) && !self.at(TokenKindName::Eof) {
            let (name, span) = self.expect_ident("expected route error name");
            let (status, _) = self.parse_status_code("expected status code after route error name");
            errors.push(RouteErrorMapping { name, status, span });
            self.eat(TokenKindName::Comma);
        }
        self.expect(TokenKindName::RBrace, "expected `}` after route errors");
        errors
    }

    fn parse_effects(&mut self) -> Vec<String> {
        self.expect(TokenKindName::LBracket, "expected `[` after `effects`");
        let mut effects = Vec::new();
        while !self.at(TokenKindName::RBracket) && !self.at(TokenKindName::Eof) {
            let (effect, _) = self.expect_ident("expected effect name");
            effects.push(effect);
            if !self.eat(TokenKindName::Comma) {
                break;
            }
        }
        self.expect(TokenKindName::RBracket, "expected `]` after effects");
        effects
    }

    fn parse_status_code(&mut self, message: &str) -> (u16, Span) {
        let token = self.advance();
        let span = token.span;
        match token.kind {
            TokenKind::Int(value) if (0..=u16::MAX as i64).contains(&value) => (value as u16, span),
            TokenKind::Int(value) => {
                self.error(
                    "EHTTP019",
                    format!("invalid HTTP status code `{value}`"),
                    span,
                    "use a three-digit HTTP status code",
                );
                (0, span)
            }
            _ => {
                self.error("EHTTP019", message, span, "use a numeric status code");
                (0, span)
            }
        }
    }

    fn parse_function(&mut self, exported: bool) -> FunctionDecl {
        let start = self
            .expect(TokenKindName::Function, "expected `function`")
            .start;
        let (name, name_span) = self.expect_ident("expected function name");
        self.expect(TokenKindName::LParen, "expected `(` after function name");

        let mut params = Vec::new();
        while !self.at(TokenKindName::RParen) && !self.at(TokenKindName::Eof) {
            let (param_name, span) = self.expect_ident("expected parameter name");
            self.expect(TokenKindName::Colon, "expected `:` after parameter name");
            let ty = self.parse_type();
            params.push(Param {
                name: param_name,
                ty,
                span,
            });
            if !self.eat(TokenKindName::Comma) {
                break;
            }
        }

        self.expect(TokenKindName::RParen, "expected `)` after parameters");
        self.expect(TokenKindName::Colon, "expected `:` before return type");
        let return_type = self.parse_type();
        let body = self.parse_block();
        let end = body
            .last()
            .map(stmt_span)
            .unwrap_or(name_span)
            .end
            .max(self.previous_span().end);

        FunctionDecl {
            exported,
            name,
            params,
            return_type,
            body,
            span: Span::new(start, end),
        }
    }

    fn parse_type_decl(&mut self, exported: bool) -> TypeDecl {
        let start = self.expect(TokenKindName::Type, "expected `type`").start;
        let (name, _) = self.expect_ident("expected type name");
        self.expect(TokenKindName::Equals, "expected `=` after type name");
        self.expect(TokenKindName::LBrace, "expected `{` to start type body");

        let mut fields = Vec::new();
        while !self.at(TokenKindName::RBrace) && !self.at(TokenKindName::Eof) {
            let (field_name, span) = self.expect_ident("expected field name");
            self.expect(TokenKindName::Colon, "expected `:` after field name");
            let ty = self.parse_type();
            fields.push(FieldDecl {
                name: field_name,
                ty,
                span,
            });
            self.eat(TokenKindName::Comma);
        }
        let end = self
            .expect(TokenKindName::RBrace, "expected `}` after type body")
            .end;

        TypeDecl {
            exported,
            name,
            fields,
            span: Span::new(start, end),
        }
    }

    fn parse_error_decl(&mut self, exported: bool) -> ErrorDecl {
        let start = self.expect(TokenKindName::Error, "expected `error`").start;
        let (name, _) = self.expect_ident("expected error name");
        self.expect(TokenKindName::LBrace, "expected `{` to start error body");

        let mut variants = Vec::new();
        while !self.at(TokenKindName::RBrace) && !self.at(TokenKindName::Eof) {
            let (variant_name, variant_span) = self.expect_ident("expected error variant name");
            self.expect(
                TokenKindName::LBrace,
                "expected `{` after error variant name",
            );
            let mut fields = Vec::new();
            while !self.at(TokenKindName::RBrace) && !self.at(TokenKindName::Eof) {
                let (field_name, span) = self.expect_ident("expected error field name");
                self.expect(TokenKindName::Colon, "expected `:` after error field name");
                let ty = self.parse_type();
                fields.push(FieldDecl {
                    name: field_name,
                    ty,
                    span,
                });
                self.eat(TokenKindName::Comma);
            }
            let end = self
                .expect(
                    TokenKindName::RBrace,
                    "expected `}` after error variant fields",
                )
                .end;
            variants.push(ErrorVariant {
                name: variant_name,
                fields,
                span: Span::new(variant_span.start, end),
            });
            self.eat(TokenKindName::Comma);
        }

        let end = self
            .expect(TokenKindName::RBrace, "expected `}` after error body")
            .end;

        ErrorDecl {
            exported,
            name,
            variants,
            span: Span::new(start, end),
        }
    }

    fn parse_block(&mut self) -> Vec<Stmt> {
        self.expect(TokenKindName::LBrace, "expected `{` to start block");
        let mut body = Vec::new();
        while !self.at(TokenKindName::RBrace) && !self.at(TokenKindName::Eof) {
            body.push(self.parse_stmt());
        }
        self.expect(TokenKindName::RBrace, "expected `}` after block");
        body
    }

    fn parse_stmt(&mut self) -> Stmt {
        match self.peek_kind() {
            TokenKind::Const => self.parse_var(false),
            TokenKind::Let => self.parse_var(true),
            TokenKind::Return => self.parse_return(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For => self.parse_for_in(),
            TokenKind::Break => self.parse_break(),
            TokenKind::Continue => self.parse_continue(),
            TokenKind::Switch => self.parse_switch(),
            TokenKind::Ident(_) if self.peek_next_is(TokenKindName::Equals) => self.parse_assign(),
            _ => {
                let expr = self.parse_expr();
                let span = expr.span();
                Stmt::Expr { expr, span }
            }
        }
    }

    fn parse_assign(&mut self) -> Stmt {
        let (name, name_span) = self.expect_ident("expected assignment target");
        self.expect(TokenKindName::Equals, "expected `=` in assignment");
        let expr = self.parse_expr();
        let span = name_span.merge(expr.span());
        Stmt::Assign { name, expr, span }
    }

    fn parse_var(&mut self, mutable: bool) -> Stmt {
        let start = if mutable {
            self.expect(TokenKindName::Let, "expected `let`").start
        } else {
            self.expect(TokenKindName::Const, "expected `const`").start
        };
        let (name, _) = self.expect_ident("expected variable name");
        let annotation = if self.eat(TokenKindName::Colon) {
            Some(self.parse_type())
        } else {
            None
        };
        self.expect(
            TokenKindName::Equals,
            "expected `=` in variable declaration",
        );
        let expr = self.parse_expr();
        let span = Span::new(start, expr.span().end);
        Stmt::Var {
            mutable,
            name,
            annotation,
            expr,
            span,
        }
    }

    fn parse_return(&mut self) -> Stmt {
        let start = self
            .expect(TokenKindName::Return, "expected `return`")
            .start;
        let expr = self.parse_expr();
        let span = Span::new(start, expr.span().end);
        Stmt::Return { expr, span }
    }

    fn parse_if(&mut self) -> Stmt {
        let start = self.expect(TokenKindName::If, "expected `if`").start;
        self.expect(TokenKindName::LParen, "expected `(` after `if`");
        let condition = self.parse_expr();
        self.expect(TokenKindName::RParen, "expected `)` after condition");
        let then_body = self.parse_block();
        let else_body = if self.eat(TokenKindName::Else) {
            self.parse_block()
        } else {
            Vec::new()
        };
        let end = else_body
            .last()
            .map(stmt_span)
            .or_else(|| then_body.last().map(stmt_span))
            .unwrap_or(condition.span())
            .end
            .max(self.previous_span().end);
        Stmt::If {
            condition,
            then_body,
            else_body,
            span: Span::new(start, end),
        }
    }

    fn parse_while(&mut self) -> Stmt {
        let start = self.expect(TokenKindName::While, "expected `while`").start;
        self.expect(TokenKindName::LParen, "expected `(` after `while`");
        let condition = self.parse_expr();
        self.expect(TokenKindName::RParen, "expected `)` after condition");
        let body = self.parse_block();
        let end = body
            .last()
            .map(stmt_span)
            .unwrap_or(condition.span())
            .end
            .max(self.previous_span().end);
        Stmt::While {
            condition,
            body,
            span: Span::new(start, end),
        }
    }

    fn parse_for_in(&mut self) -> Stmt {
        let start = self.expect(TokenKindName::For, "expected `for`").start;
        let (item, item_span) = self.expect_ident("expected loop variable name");
        self.expect(TokenKindName::In, "expected `in` after loop variable");
        let previous_suppress_struct_literal = self.suppress_struct_literal;
        self.suppress_struct_literal = true;
        let iterable = self.parse_expr();
        self.suppress_struct_literal = previous_suppress_struct_literal;
        let body = self.parse_block();
        let end = body
            .last()
            .map(stmt_span)
            .unwrap_or(iterable.span())
            .end
            .max(self.previous_span().end);
        Stmt::ForIn {
            item,
            iterable,
            body,
            span: Span::new(start, end.max(item_span.end)),
        }
    }

    fn parse_break(&mut self) -> Stmt {
        let span = self.expect(TokenKindName::Break, "expected `break`");
        Stmt::Break { span }
    }

    fn parse_continue(&mut self) -> Stmt {
        let span = self.expect(TokenKindName::Continue, "expected `continue`");
        Stmt::Continue { span }
    }

    fn parse_switch(&mut self) -> Stmt {
        let start = self
            .expect(TokenKindName::Switch, "expected `switch`")
            .start;
        self.expect(TokenKindName::LParen, "expected `(` after `switch`");
        let expr = self.parse_expr();
        self.expect(
            TokenKindName::RParen,
            "expected `)` after switch expression",
        );
        self.expect(TokenKindName::LBrace, "expected `{` to start switch body");

        let mut cases = Vec::new();
        let mut default = Vec::new();
        while !self.at(TokenKindName::RBrace) && !self.at(TokenKindName::Eof) {
            if self.at(TokenKindName::Case) {
                cases.push(self.parse_switch_case());
            } else if self.at(TokenKindName::Default) {
                let default_span = self.expect(TokenKindName::Default, "expected `default`");
                self.expect(TokenKindName::Colon, "expected `:` after `default`");
                default = self.parse_block();
                if default.is_empty() {
                    let _ = default_span;
                }
            } else {
                let token = self.peek().clone();
                self.error(
                    "E016",
                    "expected `case` or `default` in switch",
                    token.span,
                    "write `case value: { ... }` or `default: { ... }`",
                );
                self.advance();
            }
        }

        let end = self
            .expect(TokenKindName::RBrace, "expected `}` after switch body")
            .end;
        Stmt::Switch {
            expr,
            cases,
            default,
            span: Span::new(start, end),
        }
    }

    fn parse_switch_case(&mut self) -> SwitchCase {
        let start = self.expect(TokenKindName::Case, "expected `case`").start;
        let value = self.parse_expr();
        self.expect(TokenKindName::Colon, "expected `:` after case value");
        let body = self.parse_block();
        let end = body
            .last()
            .map(stmt_span)
            .unwrap_or(value.span())
            .end
            .max(self.previous_span().end);
        SwitchCase {
            value,
            body,
            span: Span::new(start, end),
        }
    }

    fn parse_expr(&mut self) -> Expr {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Expr {
        let mut expr = self.parse_and();
        while self.eat(TokenKindName::PipePipe) {
            let right = self.parse_and();
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::Or,
                right: Box::new(right),
                span,
            };
        }
        expr
    }

    fn parse_and(&mut self) -> Expr {
        let mut expr = self.parse_equality();
        while self.eat(TokenKindName::AmpAmp) {
            let right = self.parse_equality();
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::And,
                right: Box::new(right),
                span,
            };
        }
        expr
    }

    fn parse_equality(&mut self) -> Expr {
        let mut expr = self.parse_comparison();
        loop {
            let op = if self.eat(TokenKindName::EqEqEq) {
                Some(BinaryOp::Eq)
            } else if self.eat(TokenKindName::BangEqEq) {
                Some(BinaryOp::NotEq)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_comparison();
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        expr
    }

    fn parse_comparison(&mut self) -> Expr {
        let mut expr = self.parse_term();
        loop {
            let op = if self.eat(TokenKindName::LtEq) {
                Some(BinaryOp::LtEq)
            } else if self.eat(TokenKindName::GtEq) {
                Some(BinaryOp::GtEq)
            } else if self.eat(TokenKindName::Lt) {
                Some(BinaryOp::Lt)
            } else if self.eat(TokenKindName::Gt) {
                Some(BinaryOp::Gt)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_term();
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        expr
    }

    fn parse_term(&mut self) -> Expr {
        let mut expr = self.parse_factor();
        loop {
            let op = if self.eat(TokenKindName::Plus) {
                Some(BinaryOp::Add)
            } else if self.eat(TokenKindName::Minus) {
                Some(BinaryOp::Sub)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_factor();
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        expr
    }

    fn parse_factor(&mut self) -> Expr {
        let mut expr = self.parse_unary();
        loop {
            let op = if self.eat(TokenKindName::Star) {
                Some(BinaryOp::Mul)
            } else if self.eat(TokenKindName::Slash) {
                Some(BinaryOp::Div)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_unary();
            let span = expr.span().merge(right.span());
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }
        expr
    }

    fn parse_unary(&mut self) -> Expr {
        if self.eat(TokenKindName::Bang) {
            let op_span = self.previous_span();
            let expr = self.parse_unary();
            let span = op_span.merge(expr.span());
            Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
                span,
            }
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut expr = self.parse_primary();
        loop {
            if self.eat(TokenKindName::Dot) {
                let (field, field_span) = self.expect_ident("expected field name after `.`");
                let span = expr.span().merge(field_span);
                expr = Expr::FieldAccess {
                    object: Box::new(expr),
                    field,
                    span,
                };
            } else if self.at(TokenKindName::LParen) {
                expr = self.parse_postfix_call(expr);
            } else {
                break;
            }
        }
        expr
    }

    fn parse_primary(&mut self) -> Expr {
        let token = self.advance();
        match token.kind {
            TokenKind::Int(value) => Expr::Int {
                value,
                span: token.span,
            },
            TokenKind::Float(value) => Expr::Float {
                value,
                span: token.span,
            },
            TokenKind::String(value) => Expr::String {
                value,
                span: token.span,
            },
            TokenKind::True => Expr::Bool {
                value: true,
                span: token.span,
            },
            TokenKind::False => Expr::Bool {
                value: false,
                span: token.span,
            },
            TokenKind::Ident(name)
                if self.at(TokenKindName::LBrace) && !self.suppress_struct_literal =>
            {
                self.parse_struct_literal(name, token.span)
            }
            TokenKind::Ident(name) => Expr::Var {
                name,
                span: token.span,
            },
            TokenKind::Try => {
                let expr = self.parse_postfix();
                let span = token.span.merge(expr.span());
                Expr::Try {
                    expr: Box::new(expr),
                    span,
                }
            }
            TokenKind::LBrace => self.parse_object_literal(token.span),
            TokenKind::LBracket => self.parse_array_literal(token.span),
            TokenKind::LParen => {
                let expr = self.parse_expr();
                self.expect(TokenKindName::RParen, "expected `)` after expression");
                expr
            }
            _ => {
                self.error(
                    "E011",
                    "expected expression",
                    token.span,
                    "use a literal, variable, call, or object literal",
                );
                Expr::Int {
                    value: 0,
                    span: token.span,
                }
            }
        }
    }

    fn parse_array_literal(&mut self, start_span: Span) -> Expr {
        let mut elements = Vec::new();
        while !self.at(TokenKindName::RBracket) && !self.at(TokenKindName::Eof) {
            elements.push(self.parse_expr());
            if !self.eat(TokenKindName::Comma) {
                break;
            }
        }
        let end = self
            .expect(TokenKindName::RBracket, "expected `]` after array literal")
            .end;
        Expr::ArrayLiteral {
            elements,
            span: Span::new(start_span.start, end),
        }
    }

    fn parse_object_literal(&mut self, start_span: Span) -> Expr {
        let mut fields = Vec::new();
        while !self.at(TokenKindName::RBrace) && !self.at(TokenKindName::Eof) {
            if self.eat(TokenKindName::Ellipsis) {
                let expr = self.parse_expr();
                fields.push(ObjectField::Spread {
                    span: expr.span(),
                    expr,
                });
            } else {
                let (field_name, field_span) = self.expect_ident("expected object field name");
                self.expect(TokenKindName::Colon, "expected `:` after object field name");
                let expr = self.parse_expr();
                fields.push(ObjectField::Named {
                    name: field_name,
                    span: field_span.merge(expr.span()),
                    expr,
                });
            }
            self.eat(TokenKindName::Comma);
        }
        let end = self
            .expect(TokenKindName::RBrace, "expected `}` after object literal")
            .end;
        Expr::ObjectLiteral {
            fields,
            span: Span::new(start_span.start, end),
        }
    }

    fn parse_postfix_call(&mut self, callee: Expr) -> Expr {
        let start = callee.span().start;
        self.expect(TokenKindName::LParen, "expected `(` in call");
        let mut args = Vec::new();
        while !self.at(TokenKindName::RParen) && !self.at(TokenKindName::Eof) {
            args.push(self.parse_expr());
            if !self.eat(TokenKindName::Comma) {
                break;
            }
        }
        let end = self
            .expect(TokenKindName::RParen, "expected `)` after call arguments")
            .end;
        let span = Span::new(start, end);

        match callee {
            Expr::Var { name, .. } => Expr::Call {
                callee: name,
                args,
                span,
            },
            Expr::FieldAccess { object, field, .. } => {
                if let Expr::Var { name: error, .. } = *object {
                    if args.len() == 1 {
                        if let Some(fields) = object_literal_fields(&args[0]) {
                            return Expr::ErrorVariantLiteral {
                                error,
                                variant: field,
                                fields,
                                span,
                            };
                        }
                    }
                }

                self.error(
                    "E015",
                    "unsupported callee expression",
                    span,
                    "call functions as `name(...)` or construct errors as `Error.Variant({ ... })`",
                );
                Expr::Int { value: 0, span }
            }
            _ => {
                self.error(
                    "E015",
                    "unsupported callee expression",
                    span,
                    "call functions as `name(...)`",
                );
                Expr::Int { value: 0, span }
            }
        }
    }

    fn parse_struct_literal(&mut self, name: String, start_span: Span) -> Expr {
        self.expect(TokenKindName::LBrace, "expected `{` in object literal");
        let mut fields = Vec::new();
        while !self.at(TokenKindName::RBrace) && !self.at(TokenKindName::Eof) {
            let (field_name, field_span) = self.expect_ident("expected object field name");
            self.expect(TokenKindName::Colon, "expected `:` after object field name");
            let expr = self.parse_expr();
            fields.push(FieldValue {
                name: field_name,
                span: field_span.merge(expr.span()),
                expr,
            });
            self.eat(TokenKindName::Comma);
        }
        let end = self
            .expect(TokenKindName::RBrace, "expected `}` after object literal")
            .end;
        Expr::StructLiteral {
            name,
            fields,
            span: Span::new(start_span.start, end),
        }
    }

    fn parse_type(&mut self) -> Type {
        let (name, span) = self.expect_ident("expected type name");
        match name.as_str() {
            "i32" => Type::I32,
            "i64" => Type::I64,
            "u32" => Type::U32,
            "u64" => Type::U64,
            "f32" => Type::F32,
            "f64" => Type::F64,
            "bool" => Type::Bool,
            "string" => Type::String,
            "void" => Type::Void,
            "Option" => {
                self.expect(TokenKindName::Lt, "expected `<` after `Option`");
                let inner = self.parse_type();
                self.expect(TokenKindName::Gt, "expected `>` after `Option<T>`");
                Type::Option(Box::new(inner))
            }
            "Result" => {
                self.expect(TokenKindName::Lt, "expected `<` after `Result`");
                let ok = self.parse_type();
                self.expect(TokenKindName::Comma, "expected `,` in `Result<T, E>`");
                let err = self.parse_type();
                self.expect(TokenKindName::Gt, "expected `>` after `Result<T, E>`");
                Type::Result(Box::new(ok), Box::new(err))
            }
            "Array" => {
                self.expect(TokenKindName::Lt, "expected `<` after `Array`");
                let inner = self.parse_type();
                self.expect(TokenKindName::Gt, "expected `>` after `Array<T>`");
                Type::Array(Box::new(inner))
            }
            _ => {
                if name == "undefined" || name == "null" || name == "any" {
                    self.error(
                        "E012",
                        format!("unsupported Volt type `{name}`"),
                        span,
                        "use an explicit supported type",
                    );
                }
                Type::Struct(name)
            }
        }
    }

    fn expect_ident(&mut self, message: &str) -> (String, Span) {
        let token = self.advance();
        if let TokenKind::Ident(name) = token.kind {
            (name, token.span)
        } else {
            self.error("E013", message, token.span, "add an identifier here");
            ("<error>".to_string(), token.span)
        }
    }

    fn expect(&mut self, expected: TokenKindName, message: &str) -> Span {
        if self.at(expected) {
            self.advance().span
        } else {
            let span = self.peek().span;
            self.error("E014", message, span, "check the surrounding syntax");
            span
        }
    }

    fn eat(&mut self, expected: TokenKindName) -> bool {
        if self.at(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn eat_keyword(&mut self, expected: &str) -> bool {
        if matches!(self.peek_kind(), TokenKind::Ident(name) if name == expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn at(&self, expected: TokenKindName) -> bool {
        expected.matches(self.peek_kind())
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn peek_next_is(&self, expected: TokenKindName) -> bool {
        self.tokens
            .get(self.pos + 1)
            .is_some_and(|token| expected.matches(&token.kind))
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        if !matches!(token.kind, TokenKind::Eof) {
            self.pos += 1;
        }
        token
    }

    fn previous_span(&self) -> Span {
        self.tokens
            .get(self.pos.saturating_sub(1))
            .map(|token| token.span)
            .unwrap_or(Span::new(0, 0))
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

fn stmt_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::Var { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::If { span, .. }
        | Stmt::While { span, .. }
        | Stmt::ForIn { span, .. }
        | Stmt::Break { span }
        | Stmt::Continue { span }
        | Stmt::Switch { span, .. }
        | Stmt::Expr { span, .. } => *span,
    }
}

fn object_literal_fields(expr: &Expr) -> Option<Vec<FieldValue>> {
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

#[derive(Debug, Clone, Copy)]
enum TokenKindName {
    Import,
    Export,
    From,
    Function,
    Type,
    Error,
    Const,
    Let,
    Return,
    Route,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Switch,
    Case,
    Default,
    Colon,
    Comma,
    Equals,
    Dot,
    Ellipsis,
    Plus,
    Minus,
    Star,
    Slash,
    AmpAmp,
    PipePipe,
    EqEqEq,
    BangEqEq,
    Bang,
    Lt,
    Gt,
    LtEq,
    GtEq,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Eof,
}

impl TokenKindName {
    fn matches(self, kind: &TokenKind) -> bool {
        matches!(
            (self, kind),
            (TokenKindName::Function, TokenKind::Function)
                | (TokenKindName::Export, TokenKind::Export)
                | (TokenKindName::Import, TokenKind::Import)
                | (TokenKindName::From, TokenKind::From)
                | (TokenKindName::Type, TokenKind::Type)
                | (TokenKindName::Error, TokenKind::Error)
                | (TokenKindName::Const, TokenKind::Const)
                | (TokenKindName::Let, TokenKind::Let)
                | (TokenKindName::Return, TokenKind::Return)
                | (TokenKindName::Route, TokenKind::Route)
                | (TokenKindName::If, TokenKind::If)
                | (TokenKindName::Else, TokenKind::Else)
                | (TokenKindName::While, TokenKind::While)
                | (TokenKindName::For, TokenKind::For)
                | (TokenKindName::In, TokenKind::In)
                | (TokenKindName::Break, TokenKind::Break)
                | (TokenKindName::Continue, TokenKind::Continue)
                | (TokenKindName::Switch, TokenKind::Switch)
                | (TokenKindName::Case, TokenKind::Case)
                | (TokenKindName::Default, TokenKind::Default)
                | (TokenKindName::Colon, TokenKind::Colon)
                | (TokenKindName::Comma, TokenKind::Comma)
                | (TokenKindName::Equals, TokenKind::Equals)
                | (TokenKindName::Dot, TokenKind::Dot)
                | (TokenKindName::Ellipsis, TokenKind::Ellipsis)
                | (TokenKindName::Plus, TokenKind::Plus)
                | (TokenKindName::Minus, TokenKind::Minus)
                | (TokenKindName::Star, TokenKind::Star)
                | (TokenKindName::Slash, TokenKind::Slash)
                | (TokenKindName::AmpAmp, TokenKind::AmpAmp)
                | (TokenKindName::PipePipe, TokenKind::PipePipe)
                | (TokenKindName::EqEqEq, TokenKind::EqEqEq)
                | (TokenKindName::BangEqEq, TokenKind::BangEqEq)
                | (TokenKindName::Bang, TokenKind::Bang)
                | (TokenKindName::Lt, TokenKind::Lt)
                | (TokenKindName::Gt, TokenKind::Gt)
                | (TokenKindName::LtEq, TokenKind::LtEq)
                | (TokenKindName::GtEq, TokenKind::GtEq)
                | (TokenKindName::LParen, TokenKind::LParen)
                | (TokenKindName::RParen, TokenKind::RParen)
                | (TokenKindName::LBrace, TokenKind::LBrace)
                | (TokenKindName::RBrace, TokenKind::RBrace)
                | (TokenKindName::LBracket, TokenKind::LBracket)
                | (TokenKindName::RBracket, TokenKind::RBracket)
                | (TokenKindName::Eof, TokenKind::Eof)
        )
    }
}
