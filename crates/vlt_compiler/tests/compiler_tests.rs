use std::path::PathBuf;
use tempfile::tempdir;
use vlt_compiler::ast::{Decl, Expr, HttpMethod, Stmt};
use vlt_compiler::project::run_file;
use vlt_compiler::types::Type;
use vlt_compiler::{
    check_program, check_program_with_imports, generate_axum_server, generate_rust, parse_source,
    DiagnosticBag, ExternalSymbols, SourceFile,
};

fn source(text: &str) -> SourceFile {
    SourceFile::new("test.vlt", text)
}

fn parse(text: &str) -> vlt_compiler::ast::Program {
    let source = source(text);
    parse_source(&source).expect("source should parse")
}

fn check_without_entrypoint(
    program: &vlt_compiler::ast::Program,
    source: &SourceFile,
) -> Result<(), DiagnosticBag> {
    check_program_with_imports(program, source, ExternalSymbols::default(), false)
}

#[test]
fn parses_simple_function() {
    let program = parse(
        r#"
function add(a: i32, b: i32): i32 {
  return a + b
}

function main(): void {
  print(add(1, 2))
}
"#,
    );

    assert_eq!(program.declarations.len(), 2);
    let Decl::Function(function) = &program.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(function.name, "add");
    assert_eq!(function.params.len(), 2);
}

#[test]
fn parses_type_declarations() {
    let program = parse(
        r#"
type User = {
  id: u64
  email: string
}

function main(): void {
  print("ok")
}
"#,
    );

    let Decl::Type(type_decl) = &program.declarations[0] else {
        panic!("expected type declaration");
    };
    assert_eq!(type_decl.name, "User");
    assert_eq!(type_decl.fields.len(), 2);
}

#[test]
fn type_checks_correct_functions() {
    let source = source(
        r#"
function add(a: i32, b: i32): i32 {
  return a + b
}

function main(): void {
  const result = add(2, 3)
  print(result)
}
"#,
    );
    let program = parse_source(&source).unwrap();
    check_without_entrypoint(&program, &source).expect("program should check");
}

#[test]
fn type_check_reports_mismatch() {
    let source = source(
        r#"
function main(): void {
  const age: i32 = "hello"
}
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics: DiagnosticBag =
        check_program(&program, &source).expect_err("program should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "E123"));
}

#[test]
fn generates_rust_for_simple_function() {
    let program = parse(
        r#"
function add(a: i32, b: i32): i32 {
  return a + b
}

function main(): void {
  print(add(1, 2))
}
"#,
    );
    let rust = generate_rust(&program);
    assert!(rust.contains("fn add(a: i32, b: i32) -> i32"));
    assert!(rust.contains("return a + b;"));
    assert!(rust.contains("println!(\"{:?}\", add(1, 2));"));
}

#[test]
fn generated_rust_includes_targeted_lint_allows() {
    let program = parse(
        r#"
function getUserRoute(): void {
  print("ok")
}
"#,
    );
    let rust = generate_rust(&program);
    assert!(rust.contains("#![allow(non_snake_case)]"));
    assert!(rust.contains("#![allow(unused_variables)]"));
    assert!(rust.contains("#![allow(dead_code)]"));
    assert!(rust.contains("#![allow(unused_imports)]"));
}

#[test]
fn runs_hello_example() {
    let dir = tempdir().unwrap();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let example = manifest_dir.join("../../examples/hello.vlt");
    let output = run_file(&example, dir.path()).unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn struct_creation_and_field_access() {
    let source = source(
        r#"
type User = {
  id: u64
  email: string
}

function main(): void {
  const user = User {
    id: 1
    email: "carlos@test.com"
  }
  print(user.email)
}
"#,
    );
    let program = parse_source(&source).unwrap();
    check_without_entrypoint(&program, &source).expect("program should check");

    let Decl::Function(main_fn) = &program.declarations[1] else {
        panic!("expected function");
    };
    let Stmt::Expr { expr, .. } = &main_fn.body[1] else {
        panic!("expected print expression");
    };
    let Expr::Call { args, .. } = expr else {
        panic!("expected call");
    };
    assert!(matches!(&args[0], Expr::FieldAccess { .. }));
}

#[test]
fn parses_simple_health_route() {
    let program = parse(
        r#"
type HealthResponse = {
  status: string
}

route get "/health"
  ok 200 HealthResponse
{
  return ok(HealthResponse {
    status: "ok"
  })
}
"#,
    );

    let Decl::Route(route) = &program.declarations[1] else {
        panic!("expected route");
    };
    assert_eq!(route.method, HttpMethod::Get);
    assert_eq!(route.path, "/health");
    assert_eq!(route.ok_status, 200);
    assert_eq!(route.ok_type, Type::Struct("HealthResponse".to_string()));
}

#[test]
fn parses_get_route_with_path_param() {
    let program = parse(
        r#"
route get "/users/{id}"
  params { id: u64 }
  ok 200 User
{
  const user = try getUser(params.id, ctx)
  return ok(user)
}
"#,
    );

    let Decl::Route(route) = &program.declarations[0] else {
        panic!("expected route");
    };
    assert_eq!(route.method, HttpMethod::Get);
    assert_eq!(route.params[0].name, "id");
    assert_eq!(route.params[0].ty, Type::U64);
}

#[test]
fn parses_post_route_with_body() {
    let program = parse(
        r#"
route post "/users"
  body CreateUserInput
  ok 201 User
{
  const user = try createUser(body, ctx)
  return ok(user)
}
"#,
    );

    let Decl::Route(route) = &program.declarations[0] else {
        panic!("expected route");
    };
    assert_eq!(route.method, HttpMethod::Post);
    assert_eq!(
        route.body_type,
        Some(Type::Struct("CreateUserInput".to_string()))
    );
    assert_eq!(route.ok_status, 201);
}

#[test]
fn parses_patch_route_with_params_body_errors_and_effects() {
    let program = parse(
        r#"
route patch "/users/{id}"
  params { id: u64 }
  body UpdateUserInput
  ok 200 User
  errors {
    UserNotFound 404
    InvalidEmail 400
    EmailAlreadyExists 409
    DatabaseError 500
  }
  effects [db, alloc, log]
{
  const user = try updateUser({
    id: params.id,
    ...body
  }, ctx)

  return ok(user)
}
"#,
    );

    let Decl::Route(route) = &program.declarations[0] else {
        panic!("expected route");
    };
    assert_eq!(route.method, HttpMethod::Patch);
    assert_eq!(route.errors.len(), 4);
    assert_eq!(route.effects, vec!["db", "alloc", "log"]);
}

#[test]
fn invalid_route_method_fails_to_parse() {
    let source = source(
        r#"
route head "/health"
  ok 200 HealthResponse
{
  return ok(HealthResponse { status: "ok" })
}
"#,
    );
    let diagnostics = parse_source(&source).expect_err("route method should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP013"));
}

#[test]
fn missing_route_ok_fails_to_parse() {
    let source = source(
        r#"
route get "/health"
{
  return ok(HealthResponse { status: "ok" })
}
"#,
    );
    let diagnostics = parse_source(&source).expect_err("missing ok should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP012"));
}

#[test]
fn checker_rejects_missing_path_param_declaration() {
    let source = source(
        r#"
type User = { id: u64 }

route get "/users/{id}"
  ok 200 User
{
  return ok(User { id: 1 })
}
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("route should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP001"));
}

#[test]
fn checker_rejects_extra_params_field() {
    let source = source(
        r#"
type User = { id: u64 }

route get "/users/{id}"
  params { userId: u64 }
  ok 200 User
{
  return ok(User { id: 1 })
}
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("route should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP002"));
}

#[test]
fn checker_rejects_invalid_ok_status() {
    let source = source(
        r#"
type User = { id: u64 }

route get "/users/{id}"
  params { id: u64 }
  ok 99 User
{
  return ok(User { id: 1 })
}
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("route should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP004"));
}

#[test]
fn checker_rejects_invalid_error_status() {
    let source = source(
        r#"
type User = { id: u64 }

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors {
    UserNotFound 302
  }
{
  return ok(User { id: 1 })
}
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("route should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP005"));
}

#[test]
fn checker_reports_get_route_body() {
    let source = source(
        r#"
type User = { id: u64 }
type CreateUserInput = { email: string }

route get "/users"
  body CreateUserInput
  ok 200 User
{
  return ok(User { id: 1 })
}
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("route should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP003"));
}

#[test]
fn route_codegen_emits_visible_stub() {
    let program = parse(
        r#"
type User = { id: u64 }

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
{
  return ok(User { id: 1 })
}
"#,
    );
    let rust = generate_rust(&program);
    assert!(rust.contains("// Native Volt route scaffold."));
    assert!(rust.contains("route GET \"/users/{id}\" -> 200 User"));
    assert!(rust.contains("Router::new().route(\"/users/{id}\""));
}

#[test]
fn parses_option_type() {
    let program = parse(
        r#"
type User = { id: u64 }

function maybeUser(): Option<User> {
  return none
}
"#,
    );

    let Decl::Function(function) = &program.declarations[1] else {
        panic!("expected function");
    };
    assert_eq!(
        function.return_type,
        Type::Option(Box::new(Type::Struct("User".to_string())))
    );
}

#[test]
fn option_return_none_and_plain_value_check() {
    let source = source(
        r#"
type User = { id: u64 }

function missing(): Option<User> {
  return none
}

function existing(): Option<User> {
  return User({ id: 1 })
}
"#,
    );
    let program = parse_source(&source).unwrap();
    check_without_entrypoint(&program, &source).expect("program should check");
}

#[test]
fn none_from_non_option_fails() {
    let source = source(
        r#"
function main(): string {
  return none
}
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_without_entrypoint(&program, &source).expect_err("program should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "E123"));
}

#[test]
fn wrong_option_inner_type_fails() {
    let source = source(
        r#"
type User = { id: u64 }

function maybeUser(): Option<User> {
  return "wrong"
}
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_without_entrypoint(&program, &source).expect_err("program should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "E123"));
}

#[test]
fn result_option_ok_wraps_plain_value() {
    let source = source(
        r#"
type User = { id: u64 }
error UserError {
  UserNotFound { message: string }
}

function findUser(): Result<Option<User>, UserError> {
  return ok(User({ id: 1 }))
}
"#,
    );
    let program = parse_source(&source).unwrap();
    check_without_entrypoint(&program, &source).expect("program should check");
    let rust = generate_rust(&program);
    assert!(rust.contains("return Ok(Some(User { id: 1 }));"));
}

#[test]
fn option_codegen_emits_some_and_none() {
    let program = parse(
        r#"
function maybeName(): Option<string> {
  return "Carlos"
}

function noName(): Option<string> {
  return none
}
"#,
    );
    let rust = generate_rust(&program);
    assert!(rust.contains("fn maybeName() -> Option<String>"));
    assert!(rust.contains("return Some(\"Carlos\".to_string());"));
    assert!(rust.contains("return None;"));
}

#[test]
fn parses_error_declaration() {
    let program = parse(
        r#"
export error UserError {
  UserNotFound { message: string }
}
"#,
    );

    let Decl::Error(error_decl) = &program.declarations[0] else {
        panic!("expected error declaration");
    };
    assert!(error_decl.exported);
    assert_eq!(error_decl.name, "UserError");
    assert_eq!(error_decl.variants[0].name, "UserNotFound");
}

#[test]
fn error_variant_construction_checks_and_generates() {
    let source = source(
        r#"
error UserError {
  UserNotFound { message: string }
}

function fail(): Result<string, UserError> {
  return err(UserError.UserNotFound({ message: "User not found" }))
}
"#,
    );
    let program = parse_source(&source).unwrap();
    check_without_entrypoint(&program, &source).expect("program should check");
    let rust = generate_rust(&program);
    assert!(rust.contains("enum UserError"));
    assert!(rust.contains("UserError::UserNotFound { message: \"User not found\".to_string() }"));
}

#[test]
fn error_variant_validation_fails() {
    for text in [
        r#"
error UserError { UserNotFound { message: string } }
function fail(): Result<string, UserError> {
  return err(UserError.Unknown({ message: "x" }))
}
"#,
        r#"
error UserError { UserNotFound { message: string } }
function fail(): Result<string, UserError> {
  return err(UserError.UserNotFound({}))
}
"#,
        r#"
error UserError { UserNotFound { message: string } }
function fail(): Result<string, UserError> {
  return err(UserError.UserNotFound({ message: 123 }))
}
"#,
        r#"
error UserError { UserNotFound { message: string } }
function fail(): Result<string, UserError> {
  return err(UserError.UserNotFound({ message: "x", extra: "wrong" }))
}
"#,
    ] {
        let source = source(text);
        let program = parse_source(&source).unwrap();
        check_without_entrypoint(&program, &source).expect_err("program should fail");
    }
}

#[test]
fn option_if_narrowing_checks() {
    let source = source(
        r#"
type User = { email: string }

function maybeUser(): Option<User> {
  return User({ email: "demo@test.com" })
}

function main(): void {
  const user = maybeUser()
  if (user) {
    print(user.email)
  }
}
"#,
    );
    let program = parse_source(&source).unwrap();
    check_without_entrypoint(&program, &source).expect("program should check");
}

#[test]
fn option_field_access_outside_narrowing_fails() {
    let source = source(
        r#"
type User = { email: string }

function maybeUser(): Option<User> {
  return none
}

function main(): void {
  const user = maybeUser()
  if (user) {
    print(user.email)
  }
  print(user.email)
}
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_without_entrypoint(&program, &source).expect_err("program should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "E109"));
}

#[test]
fn if_rejects_string_and_number_conditions() {
    for text in [
        r#"function main(): void { if ("hello") { print("x") } }"#,
        r#"function main(): void { if (123) { print("x") } }"#,
    ] {
        let source = source(text);
        let program = parse_source(&source).unwrap();
        let diagnostics =
            check_without_entrypoint(&program, &source).expect_err("program should fail");
        assert!(diagnostics
            .all()
            .iter()
            .any(|diagnostic| diagnostic.code == "E125"));
    }
}

#[test]
fn negative_option_guard_narrows_after_return() {
    let source = source(
        r#"
type User = { email: string }
error UserError {
  UserNotFound { message: string }
}

function maybeUser(): Result<Option<User>, UserError> {
  return ok(none)
}

function getUser(): Result<User, UserError> {
  const user = try maybeUser()
  if (!user) {
    return err(UserError.UserNotFound({ message: "User not found" }))
  }
  return ok(user)
}
"#,
    );
    let program = parse_source(&source).unwrap();
    check_without_entrypoint(&program, &source).expect("program should check");
    let rust = generate_rust(&program);
    assert!(rust.contains("let user = if let Some(user) = user"));
}

#[test]
fn positive_option_narrowing_codegen_uses_if_let() {
    let program = parse(
        r#"
type User = { email: string }

function maybeUser(): Option<User> {
  return User({ email: "demo@test.com" })
}

function main(): void {
  const user = maybeUser()
  if (user) {
    print(user.email)
  }
}
"#,
    );
    let rust = generate_rust(&program);
    assert!(rust.contains("if let Some(user) = user"));
}

#[test]
fn parses_typed_route_errors() {
    let program = parse(
        r#"
route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors UserError {
    UserNotFound 404
  }
{
  return ok(user)
}
"#,
    );

    let Decl::Route(route) = &program.declarations[0] else {
        panic!("expected route");
    };
    assert_eq!(
        route.error_type,
        Some(Type::Struct("UserError".to_string()))
    );
    assert_eq!(route.errors[0].name, "UserNotFound");
}

#[test]
fn typed_route_error_validation() {
    let source = source(
        r#"
type User = { id: u64 }
error UserError {
  UserNotFound { message: string }
}

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors UserError {
    Unknown 404
  }
{
  return ok(User({ id: params.id }))
}
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("program should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP009"));
}

#[test]
fn typed_route_unknown_error_type_fails() {
    let source = source(
        r#"
type User = { id: u64 }

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors MissingError {
    UserNotFound 404
  }
{
  return ok(User({ id: params.id }))
}
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("program should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP008"));
}

#[test]
fn typed_route_non_error_type_fails() {
    let source = source(
        r#"
type User = { id: u64 }
type UserError = { message: string }

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors UserError {
    UserNotFound 404
  }
{
  return ok(User({ id: params.id }))
}
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("program should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP007"));
}

#[test]
fn parses_route_with_handler_and_no_inline_body() {
    let program = parse(
        r#"
type User = { id: u64 }

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  handler getUserRoute
"#,
    );

    let Decl::Route(route) = &program.declarations[1] else {
        panic!("expected route");
    };
    assert_eq!(route.handler.as_deref(), Some("getUserRoute"));
    assert!(route.statements.is_empty());
}

#[test]
fn parser_reports_handler_without_identifier() {
    let source = source(
        r#"
type User = { id: u64 }

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  handler
"#,
    );
    let diagnostics = parse_source(&source).expect_err("handler name should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP011"));
}

#[test]
fn parser_reports_handler_and_inline_body() {
    let source = source(
        r#"
type User = { id: u64 }

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  handler getUserRoute
{
  return ok(User({ id: params.id }))
}
"#,
    );
    let diagnostics = parse_source(&source).expect_err("handler plus body should fail");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP014"));
}

#[test]
fn parser_reports_route_without_handler_or_body() {
    let source = source(
        r#"
type User = { id: u64 }

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
"#,
    );
    let diagnostics = parse_source(&source).expect_err("route body should be required");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP015"));
}

#[test]
fn checker_reports_unknown_route_handler() {
    let source = source(
        r#"
type User = { id: u64 }

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  handler getUserRoute
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("handler should be unknown");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP016"));
}

#[test]
fn checker_reports_route_handler_return_mismatch() {
    let source = source(
        r#"
type User = { id: u64 }
type Other = { id: u64 }
error UserError { UserNotFound { message: string } }

function getUserRoute(params: GetUsersIdParams, ctx: Ctx): Result<Other, UserError> {
  return ok(Other({ id: params.id }))
}

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors UserError { UserNotFound 404 }
  handler getUserRoute
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("return should mismatch");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP017"));
}

#[test]
fn checker_reports_route_handler_error_type_mismatch() {
    let source = source(
        r#"
type User = { id: u64 }
error UserError { UserNotFound { message: string } }
error OtherError { Other { message: string } }

function getUserRoute(params: GetUsersIdParams, ctx: Ctx): Result<User, OtherError> {
  return err(OtherError.Other({ message: "wrong" }))
}

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors UserError { UserNotFound 404 }
  handler getUserRoute
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("error type should mismatch");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP017"));
}

#[test]
fn checker_reports_route_handler_params_mismatch() {
    let source = source(
        r#"
type User = { id: u64 }
error UserError { UserNotFound { message: string } }

function getUserRoute(ctx: Ctx): Result<User, UserError> {
  return ok(User({ id: 1 }))
}

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors UserError { UserNotFound 404 }
  handler getUserRoute
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("params should mismatch");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP018"));
}

#[test]
fn checker_reports_route_handler_missing_body() {
    let source = source(
        r#"
type User = { id: u64 }
type UpdateUserInput = { name: string }
error UserError { UserNotFound { message: string } }

function updateUserRoute(params: PatchUsersIdParams, ctx: Ctx): Result<User, UserError> {
  return ok(User({ id: params.id }))
}

route patch "/users/{id}"
  params { id: u64 }
  body UpdateUserInput
  ok 200 User
  errors UserError { UserNotFound 404 }
  handler updateUserRoute
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("body should be required");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP018"));
}

#[test]
fn checker_reports_route_handler_missing_ctx() {
    let source = source(
        r#"
type User = { id: u64 }
error UserError { UserNotFound { message: string } }

function getUserRoute(params: GetUsersIdParams): Result<User, UserError> {
  return ok(User({ id: params.id }))
}

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors UserError { UserNotFound 404 }
  handler getUserRoute
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("ctx should be required");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP018"));
}

#[test]
fn checker_reports_route_handler_wrong_argument_order() {
    let source = source(
        r#"
type User = { id: u64 }
type UpdateUserInput = { name: string }
error UserError { UserNotFound { message: string } }

function updateUserRoute(
  body: UpdateUserInput,
  params: PatchUsersIdParams,
  ctx: Ctx
): Result<User, UserError> {
  return ok(User({ id: params.id }))
}

route patch "/users/{id}"
  params { id: u64 }
  body UpdateUserInput
  ok 200 User
  errors UserError { UserNotFound 404 }
  handler updateUserRoute
"#,
    );
    let program = parse_source(&source).unwrap();
    let diagnostics = check_program(&program, &source).expect_err("argument order should mismatch");
    assert!(diagnostics
        .all()
        .iter()
        .any(|diagnostic| diagnostic.code == "EHTTP018"));
}

#[test]
fn checker_accepts_valid_typed_route_handler() {
    let source = source(
        r#"
type User = { id: u64 }
type UpdateUserInput = { name: string }
type UsersPage = { page: u32 }
error UserError { UserNotFound { message: string } }

function updateUserRoute(
  params: PatchUsersIdParams,
  query: PatchUsersIdQuery,
  body: UpdateUserInput,
  ctx: Ctx
): Result<User, UserError> {
  return ok(User({ id: params.id }))
}

route patch "/users/{id}"
  params { id: u64 }
  query { page: u32 }
  body UpdateUserInput
  ok 200 User
  errors UserError { UserNotFound 404 }
  effects [db, log]
  handler updateUserRoute
"#,
    );
    let program = parse_source(&source).unwrap();
    check_program(&program, &source).expect("valid handler route should check");
}

#[test]
fn axum_codegen_emits_external_handler_wrapper() {
    let source = source(
        r#"
type User = { id: u64 }
type UpdateUserInput = { name: string }
error UserError { UserNotFound { message: string } DatabaseError { message: string } }

function updateUserRoute(
  params: PatchUsersIdParams,
  body: UpdateUserInput,
  ctx: Ctx
): Result<User, UserError> {
  return ok(User({ id: params.id }))
}

route patch "/users/{id}"
  params { id: u64 }
  body UpdateUserInput
  ok 200 User
  errors UserError {
    UserNotFound 404
    DatabaseError 500
  }
  handler updateUserRoute
"#,
    );
    let program = parse_source(&source).unwrap();
    check_program(&program, &source).expect("program should check");
    let rust = generate_axum_server(&program, &source).expect("axum should lower");
    assert!(rust.contains("#![allow(non_snake_case)]"));
    assert!(rust.contains("pub struct PatchUsersIdParams"));
    assert!(rust.contains("fn updateUserRoute(params: PatchUsersIdParams, body: UpdateUserInput, ctx: RequestCtx) -> Result<User, UserError>"));
    assert!(rust.contains("match updateUserRoute(params, body, ctx)"));
    assert!(rust.contains("Ok(value) => (StatusCode::OK, Json(value)).into_response()"));
    assert!(rust.contains("let status = patch_users_id_route_error_status(&error);"));
    assert!(rust.contains("(status, Json(error)).into_response()"));
}
