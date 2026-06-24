use std::path::PathBuf;
use tempfile::tempdir;
use vlt_compiler::ast::{Decl, Expr, HttpMethod, Stmt};
use vlt_compiler::project::run_file;
use vlt_compiler::types::Type;
use vlt_compiler::{check_program, generate_rust, parse_source, DiagnosticBag, SourceFile};

fn source(text: &str) -> SourceFile {
    SourceFile::new("test.vlt", text)
}

fn parse(text: &str) -> vlt_compiler::ast::Program {
    let source = source(text);
    parse_source(&source).expect("source should parse")
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
    check_program(&program, &source).expect("program should check");
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
    check_program(&program, &source).expect("program should check");

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
