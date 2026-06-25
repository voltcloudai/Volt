use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use tempfile::tempdir;

fn vlt() -> &'static str {
    env!("CARGO_BIN_EXE_vlt")
}

fn run(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(vlt())
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("vlt should run")
}

#[test]
fn new_api_creates_expected_files() {
    let dir = tempdir().unwrap();
    let output = run(&["new", "api", "my-api"], dir.path());
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let root = dir.path().join("my-api");
    for file in [
        "volt.toml",
        "AGENTS.md",
        "CLAUDE.md",
        "README.md",
        "src/main.vlt",
        "src/health/health.routes.vlt",
        "src/health/health.types.vlt",
        "src/users/users.routes.vlt",
        "src/users/users.service.vlt",
        "src/users/users.repository.vlt",
        "src/users/users.types.vlt",
        "src/users/users.errors.vlt",
        "src/users/users.test.vlt",
        ".ai/project.md",
        ".ai/symbols.json",
        ".ai/routes.json",
        ".ai/source-map.json",
        ".ai/tasks",
        ".ai/files",
    ] {
        assert!(root.join(file).exists(), "missing {file}");
    }

    let users_routes = std::fs::read_to_string(root.join("src/users/users.routes.vlt")).unwrap();
    assert!(users_routes.contains("route get \"/users/{id}\""));
    assert!(users_routes.contains("params { id: u64 }"));
    assert!(users_routes.contains("handler getUserRoute"));
    assert!(users_routes.contains("handler createUserRoute"));
    assert!(!users_routes.contains("app.get("));
}

#[test]
fn ai_index_generates_symbols_json() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");

    let output = run(&["ai", "index"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let symbols = std::fs::read_to_string(root.join(".ai/symbols.json")).unwrap();
    assert!(symbols.contains("\"language\": \"Volt\""));
    assert!(symbols.contains("\"name\": \"User\""));
    assert!(symbols.contains("\"name\": \"createUserRoute\""));
    assert!(symbols.contains("\"name\": \"UserError\""));
    assert!(symbols.contains("\"name\": \"UserNotFound\""));
    assert!(symbols.contains("\"fields\""));
    assert!(symbols.contains("\"module\": \"users\""));
}

#[test]
fn generated_main_can_be_checked() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");

    let output = run(&["check", "src/main.vlt"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn generated_api_builds_axum_project_and_serves_health() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");

    let output = run(&["build"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let generated = std::fs::read_to_string(root.join("target/volt/rust-project/src/main.rs"))
        .expect("generated Rust should exist");
    assert!(generated.contains("#![allow(non_snake_case)]"));
    assert!(generated.contains("#![allow(unused_variables)]"));
    assert!(generated.contains("#![allow(dead_code)]"));
    assert!(generated.contains("axum::Router"));
    assert!(generated.contains("tokio::main"));
    assert!(generated.contains("axum::serve"));
    assert!(generated.contains(".route(\"/health\", get(get_health_route))"));
    assert!(generated.contains("async fn get_health_route"));
    assert!(generated.contains("async fn get_users_id_route"));
    assert!(generated.contains("async fn post_users_route"));
    assert!(generated.contains("match getUserRoute(params, ctx)"));
    assert!(generated.contains("match createUserRoute(body, ctx)"));

    let cargo_check = Command::new("cargo")
        .arg("check")
        .current_dir(root.join("target/volt/rust-project"))
        .output()
        .expect("cargo check should run");
    assert!(
        cargo_check.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&cargo_check.stderr)
    );

    let port = free_port();
    let mut child = Command::new(root.join("target/volt/my-api"))
        .env("VLT_ADDR", format!("127.0.0.1:{port}"))
        .spawn()
        .expect("server should start");
    let response = request_path(port, "/health", &mut child);
    assert!(response.contains("200 OK"), "{response}");
    assert!(response.contains(r#"{"status":"ok"}"#), "{response}");
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn generated_api_builds_with_phase_4_1_features() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    std::fs::write(
        root.join("src/users/users.phase41.vlt"),
        r#"import { User } from "./users.types"

function canUpdate(isAdmin: bool, isOwner: bool): bool {
  return isAdmin || isOwner
}

function countAttempts(): i32 {
  let attempts = 0
  attempts = attempts + 1
  return attempts
}

function defaultIds(): Array<u64> {
  return [1, 2, 3]
}

function emptyIds(): Array<u64> {
  const ids: Array<u64> = []
  return ids
}

function demoUsers(): Array<User> {
  return [User({ id: 1, email: "demo@test.com", name: "Demo User" })]
}
"#,
    )
    .unwrap();

    let output = run(&["build"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let generated = std::fs::read_to_string(root.join("target/volt/rust-project/src/main.rs"))
        .expect("generated Rust should exist");
    assert!(generated.contains("let mut attempts = 0;"));
    assert!(generated.contains("attempts = attempts + 1;"));
    assert!(generated.contains("return isAdmin || isOwner;"));
    assert!(generated.contains("fn defaultIds() -> Vec<u64>"));
    assert!(generated.contains("return vec![1, 2, 3];"));
    assert!(generated.contains("let ids: Vec<u64> = vec![];"));

    let cargo_check = Command::new("cargo")
        .arg("check")
        .current_dir(root.join("target/volt/rust-project"))
        .output()
        .expect("cargo check should run");
    assert!(
        cargo_check.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&cargo_check.stderr)
    );
}

#[test]
fn generated_api_builds_with_phase_4_2_control_flow() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    std::fs::write(
        root.join("src/users/users.phase42.vlt"),
        r#"type RoleUser = {
  id: u64
  email: string
  role: string
}

function countAdmins(users: Array<RoleUser>): i32 {
  let count = 0
  for user in users {
    switch (user.role) {
      case "admin": {
        count = count + 1
      }
      default: {
        continue
      }
    }
  }
  return count
}

function waitUntilReady(maxAttempts: i32): i32 {
  let attempts = 0
  while (attempts < maxAttempts) {
    attempts = attempts + 1
    if (attempts === 3) {
      break
    }
  }
  return attempts
}
"#,
    )
    .unwrap();

    let output = run(&["build"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let generated = std::fs::read_to_string(root.join("target/volt/rust-project/src/main.rs"))
        .expect("generated Rust should exist");
    assert!(generated.contains("for user in users {"));
    assert!(generated.contains("if user.role == \"admin\".to_string() {"));
    assert!(generated.contains("continue;"));
    assert!(generated.contains("while attempts < maxAttempts {"));
    assert!(generated.contains("break;"));

    let cargo_check = Command::new("cargo")
        .arg("check")
        .current_dir(root.join("target/volt/rust-project"))
        .output()
        .expect("cargo check should run");
    assert!(
        cargo_check.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&cargo_check.stderr)
    );
}

#[test]
fn unsupported_route_body_fails_before_rust_is_generated() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    std::fs::write(
        root.join("src/health/health.routes.vlt"),
        r#"import { HealthResponse } from "./health.types"

route get "/health"
  ok 200 HealthResponse
{
  if (true) {
    return ok(HealthResponse {
      status: "ok"
    })
  }
}
"#,
    )
    .unwrap();

    let output = run(&["build"], &root);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("EHTTPLOWER001"), "{stderr}");
    assert!(stderr.contains("handler myRouteHandler"), "{stderr}");
}

#[test]
fn ai_summary_prints_project_summary() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    assert!(run(&["ai", "index"], &root).status.success());

    let output = run(&["ai", "summary"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# Volt AI Context"));
    assert!(stdout.contains("# Volt Project Summary"));
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn request_path(port: u16, path: &str, child: &mut Child) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
            let request =
                format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
            stream.write_all(request.as_bytes()).unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            return response;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("server did not accept connections");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn write_realistic_users_module(root: &Path) {
    std::fs::write(
        root.join("src/users/users.types.vlt"),
        r#"export type User = {
  id: u64
  email: string
  name: string
}

export type UpdateUserInput = {
  email: string
  name: string
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("src/users/users.errors.vlt"),
        r#"export error UserError {
  UserNotFound { message: string }
  InvalidEmail { message: string }
  EmailAlreadyExists { email: string }
  DatabaseError { message: string }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("src/users/users.repository.vlt"),
        r#"import { User } from "./users.types"
import { UserError } from "./users.errors"

export function findUserById(
  id: u64,
  ctx: Ctx
): Result<Option<User>, UserError> {
  if (id === 1) {
    return ok(User({
      id: 1,
      email: "demo@test.com",
      name: "Demo User"
    }))
  }

  return ok(none)
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("src/users/users.service.vlt"),
        r#"import { User, UpdateUserInput } from "./users.types"
import { UserError } from "./users.errors"
import { findUserById } from "./users.repository"

export function updateUserRoute(
  params: PatchUsersIdParams,
  body: UpdateUserInput,
  ctx: Ctx
): Result<User, UserError> {
  const user = try findUserById(params.id, ctx)

  if (!user) {
    return err(UserError.UserNotFound({
      message: "User not found"
    }))
  }

  return ok(User({
    id: user.id,
    email: body.email,
    name: body.name
  }))
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("src/users/users.routes.vlt"),
        r#"import { User, UpdateUserInput } from "./users.types"
import { UserError } from "./users.errors"
import { updateUserRoute } from "./users.service"

route patch "/users/{id}"
  params { id: u64 }
  body UpdateUserInput
  ok 200 User
  errors UserError {
    UserNotFound 404
    InvalidEmail 400
    EmailAlreadyExists 409
    DatabaseError 500
  }
  effects [db, log]
  handler updateUserRoute
"#,
    )
    .unwrap();
}

#[test]
fn realistic_multifile_handler_api_builds_and_generates_dependencies() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    write_realistic_users_module(&root);

    let index = run(&["ai", "index"], &root);
    assert!(
        index.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&index.stderr)
    );

    let routes = std::fs::read_to_string(root.join(".ai/routes.json")).unwrap();
    assert!(routes.contains("\"method\": \"PATCH\""));
    assert!(routes.contains("\"path\": \"/users/{id}\""));
    assert!(routes.contains("\"body\": \"UpdateUserInput\""));
    assert!(routes.contains("\"errorType\": \"UserError\""));
    assert!(routes.contains("\"effects\""));
    assert!(routes.contains("\"db\""));
    assert!(routes.contains("\"log\""));
    assert!(routes.contains("\"handler\": \"updateUserRoute\""));

    let source_map = std::fs::read_to_string(root.join(".ai/source-map.json")).unwrap();
    assert!(source_map.contains("\"path\": \"src/users/users.routes.vlt\""));
    assert!(source_map.contains("\"path\": \"src/users/users.service.vlt\""));
    assert!(source_map.contains("\"path\": \"src/users/users.repository.vlt\""));

    let explain = run(&["explain", "updateUserRoute"], &root);
    assert!(
        explain.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&explain.stderr)
    );
    let explain_stdout = String::from_utf8_lossy(&explain.stdout);
    assert!(explain_stdout.contains("Related routes: [\"PATCH /users/{id}\"]"));

    let output = run(&["build"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let generated = std::fs::read_to_string(root.join("target/volt/rust-project/src/main.rs"))
        .expect("generated Rust should exist");
    assert!(generated.contains("#![allow(non_snake_case)]"));
    assert!(generated.contains("pub struct PatchUsersIdParams"));
    assert!(generated.contains("fn findUserById"));
    assert!(generated.contains("fn updateUserRoute"));
    assert!(generated.contains("async fn patch_users_id_route"));
    assert!(generated.contains("match updateUserRoute(params, body, ctx)"));
    assert!(generated.contains("patch_users_id_route_error_status"));
    assert!(generated.contains("UserError::UserNotFound { .. } => StatusCode::NOT_FOUND"));
    assert!(generated.contains("let user = if let Some(user) = user"));

    let cargo_check = Command::new("cargo")
        .arg("check")
        .current_dir(root.join("target/volt/rust-project"))
        .output()
        .expect("cargo check should run");
    assert!(
        cargo_check.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&cargo_check.stderr)
    );
}

#[test]
fn handler_route_typed_error_returns_http_status() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    std::fs::write(
        root.join("src/users/users.types.vlt"),
        r#"export type User = {
  id: u64
  email: string
  name: string
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("src/users/users.errors.vlt"),
        r#"export error UserError {
  UserNotFound { message: string }
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("src/users/users.repository.vlt"),
        r#"function usersRepositoryPlaceholder(): void {
  print("placeholder")
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("src/users/users.service.vlt"),
        r#"import { User } from "./users.types"
import { UserError } from "./users.errors"

export function getUserRoute(
  params: GetUsersIdParams,
  ctx: Ctx
): Result<User, UserError> {
  return err(UserError.UserNotFound({
    message: "User not found"
  }))
}
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("src/users/users.routes.vlt"),
        r#"import { User } from "./users.types"
import { UserError } from "./users.errors"
import { getUserRoute } from "./users.service"

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors UserError {
    UserNotFound 404
  }
  handler getUserRoute
"#,
    )
    .unwrap();

    let output = run(&["build"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let port = free_port();
    let mut child = Command::new(root.join("target/volt/my-api"))
        .env("VLT_ADDR", format!("127.0.0.1:{port}"))
        .spawn()
        .expect("server should start");
    let response = request_path(port, "/users/999", &mut child);
    assert!(response.contains("404 Not Found"), "{response}");
    assert!(response.contains("UserNotFound"), "{response}");
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn ai_index_generates_routes_and_source_map() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    assert!(run(&["ai", "index"], &root).status.success());

    let routes = std::fs::read_to_string(root.join(".ai/routes.json")).unwrap();
    assert!(routes.contains("\"method\": \"GET\""));
    assert!(routes.contains("\"path\": \"/users/{id}\""));
    assert!(routes.contains("\"module\": \"users\""));
    assert!(routes.contains("\"params\""));
    assert!(routes.contains("\"id\": \"u64\""));
    assert!(routes.contains("\"success\""));
    assert!(routes.contains("\"status\": 200"));
    assert!(routes.contains("\"type\": \"User\""));
    assert!(routes.contains("\"handler\": \"getUserRoute\""));

    let source_map = std::fs::read_to_string(root.join(".ai/source-map.json")).unwrap();
    assert!(source_map.contains("\"path\": \"src/users/users.service.vlt\""));
    assert!(source_map.contains("\"kind\": \"service\""));
    assert!(source_map.contains("Business logic for the users module."));
    assert!(root
        .join(".ai/files/src_users_users_service.summary.md")
        .exists());
}

#[test]
fn explain_finds_known_symbol() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    assert!(run(&["ai", "index"], &root).status.success());

    let output = run(&["explain", "createUserRoute"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# Symbol: createUserRoute"));
    assert!(stdout.contains("function createUserRoute"));
    assert!(stdout.contains("Related routes: [\"POST /users\"]"));
}

#[test]
fn plan_update_user_endpoint_uses_patch_and_users_files() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    assert!(run(&["ai", "index"], &root).status.success());

    let output = run(&["plan", "add new update user endpoint"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("route patch \"/users/{id}\""));
    assert!(stdout.contains("handler updateUserRoute"));
    assert!(stdout.contains("Confidence:\nhigh"));
    assert!(stdout.contains("Inferred intent:\nUpdate an existing user."));
    assert!(stdout.contains("src/users/users.routes.vlt"));
    assert!(stdout.contains("src/users/users.service.vlt"));
    assert!(stdout.contains("errors UserError {"));
    assert!(stdout.contains("EmailAlreadyExists 409"));
    assert!(stdout.contains("Existing users module found."));
}

#[test]
fn planner_reports_low_confidence_for_unknown_modules() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    assert!(run(&["ai", "index"], &root).status.success());

    let output = run(&["plan", "add billing report endpoint"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Confidence:\nlow"));
    assert!(stdout.contains("No existing module matching \"billing\" was found."));
}

#[test]
fn route_metadata_comments_are_parsed() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    std::fs::write(
        root.join("src/users/users.routes.vlt"),
        r#"// @route PATCH /users/:id
// @handler updateUserHandler
// @input UpdateUserInput
// @output User
// @errors UpdateUserError DatabaseError
function updateUserHandler(input: UpdateUserInput): Result<User, UpdateUserError> {
  return err("not implemented")
}
"#,
    )
    .unwrap();

    assert!(run(&["ai", "index"], &root).status.success());
    let routes = std::fs::read_to_string(root.join(".ai/routes.json")).unwrap();
    assert!(routes.contains("\"method\": \"PATCH\""));
    assert!(routes.contains("\"path\": \"/users/:id\""));
    assert!(routes.contains("\"handler\": \"updateUserHandler\""));
    assert!(routes.contains("\"UpdateUserError\""));
}

#[test]
fn native_route_index_includes_structured_metadata() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    std::fs::write(
        root.join("src/users/users.routes.vlt"),
        r#"route patch "/users/{id}"
  params { id: u64 }
  body UpdateUserInput
  ok 200 User
  errors UserError {
    UserNotFound 404
    InvalidEmail 400
    EmailAlreadyExists 409
    DatabaseError 500
  }
  effects [db, alloc, log]
  handler updateUserRoute
"#,
    )
    .unwrap();

    assert!(run(&["ai", "index"], &root).status.success());
    let routes = std::fs::read_to_string(root.join(".ai/routes.json")).unwrap();
    assert!(routes.contains("\"method\": \"PATCH\""));
    assert!(routes.contains("\"path\": \"/users/{id}\""));
    assert!(routes.contains("\"params\""));
    assert!(routes.contains("\"id\": \"u64\""));
    assert!(routes.contains("\"body\": \"UpdateUserInput\""));
    assert!(routes.contains("\"success\""));
    assert!(routes.contains("\"status\": 200"));
    assert!(routes.contains("\"type\": \"User\""));
    assert!(routes.contains("\"errorType\": \"UserError\""));
    assert!(routes.contains("\"name\": \"InvalidEmail\""));
    assert!(routes.contains("\"status\": 400"));
    assert!(routes.contains("\"effects\""));
    assert!(routes.contains("\"db\""));
    assert!(routes.contains("\"alloc\""));
    assert!(routes.contains("\"log\""));
    assert!(routes.contains("\"handler\": \"updateUserRoute\""));

    let source_map = std::fs::read_to_string(root.join(".ai/source-map.json")).unwrap();
    assert!(source_map.contains("\"routes\""));
    assert!(source_map.contains("\"path\": \"/users/{id}\""));
}

#[test]
fn native_routes_have_priority_over_route_comments() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    std::fs::write(
        root.join("src/users/users.routes.vlt"),
        r#"// @route PATCH /users/{id}
// @handler legacyUpdateUser
// @input LegacyInput
// @output LegacyUser
// @errors LegacyError
route patch "/users/{id}"
  params { id: u64 }
  body UpdateUserInput
  ok 200 User
  errors {
    UserNotFound 404
  }
{
  const user = try updateUser(body, ctx)
  return ok(user)
}
"#,
    )
    .unwrap();

    assert!(run(&["ai", "index"], &root).status.success());
    let routes = std::fs::read_to_string(root.join(".ai/routes.json")).unwrap();
    assert!(routes.contains("\"body\": \"UpdateUserInput\""));
    assert!(routes.contains("\"type\": \"User\""));
    assert!(!routes.contains("LegacyUser"));
    assert!(!routes.contains("legacyUpdateUser"));
}

#[test]
fn basic_app_route_calls_are_parsed() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    std::fs::write(
        root.join("src/users/users.extra.routes.vlt"),
        r#"app.get("/users", listUsers)
app.post("/users", createUserHandler)
app.patch("/users/:id", updateUserHandler)
app.delete("/users/:id", deleteUserHandler)
"#,
    )
    .unwrap();

    assert!(run(&["ai", "index"], &root).status.success());
    let routes = std::fs::read_to_string(root.join(".ai/routes.json")).unwrap();
    assert!(routes.contains("\"method\": \"GET\""));
    assert!(routes.contains("\"method\": \"POST\""));
    assert!(routes.contains("\"method\": \"PATCH\""));
    assert!(routes.contains("\"method\": \"DELETE\""));
    assert!(routes.contains("\"path\": \"/users/:id\""));
    assert!(routes.contains("\"handler\": \"updateUserHandler\""));
}

#[test]
fn ai_prompt_generates_useful_generic_prompt() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    assert!(run(&["ai", "index"], &root).status.success());

    let output = run(&["ai", "prompt", "add new update user endpoint"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# AI implementation prompt"));
    assert!(stdout.contains("## Inferred intent\n\nUpdate an existing user."));
    assert!(stdout.contains("route patch \"/users/{id}\""));
    assert!(stdout.contains("handler updateUserRoute"));
    assert!(stdout.contains("src/users/users.routes.vlt"));
    assert!(stdout.contains("src/users/users.service.vlt"));
    assert!(stdout.contains("Treat route declarations as HTTP contracts"));
    assert!(stdout.contains("Edit `*.routes.vlt` for HTTP contract changes."));
    assert!(stdout.contains("Edit `*.service.vlt` or the handler function for business logic."));
    assert!(stdout.contains("## Validation commands"));
    assert!(stdout.contains("vlt ai index"));
    assert!(stdout.contains("vlt build"));
    assert!(stdout.contains("Use Result<T, E> for fallible operations."));
    assert!(stdout.contains("Use Option<T> for absence"));
    assert!(stdout.contains("Use `let` for mutable local variables."));
    assert!(stdout.contains("Use `Array<T>` for arrays."));
    assert!(stdout.contains("Empty arrays require contextual type"));
    assert!(stdout.contains("Use `while (condition) { ... }`"));
    assert!(stdout.contains("Use `for item in array { ... }`"));
    assert!(stdout.contains("Use normal TypeScript-like `switch`"));
    assert!(stdout.contains("No fallthrough in switch."));
    assert!(stdout.contains("errors UserError {"));
    assert!(stdout.contains("EmailAlreadyExists 409"));
    assert!(stdout.contains("Prefer native routes over `app.get(...)` or `app.patch(...)` calls."));
    assert!(stdout.contains("Do not call external AI APIs."));
}

#[test]
fn ai_prompt_codex_format_mentions_agents() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    assert!(run(&["ai", "index"], &root).status.success());

    let output = run(
        &[
            "ai",
            "prompt",
            "add new update user endpoint",
            "--format",
            "codex",
        ],
        &root,
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("## Codex instructions"));
    assert!(stdout.contains("AGENTS.md"));
}

#[test]
fn ai_prompt_claude_format_mentions_claude_md() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    assert!(run(&["ai", "index"], &root).status.success());

    let output = run(
        &[
            "ai",
            "prompt",
            "add new update user endpoint",
            "--format",
            "claude",
        ],
        &root,
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("## Claude Code instructions"));
    assert!(stdout.contains("CLAUDE.md"));
}

#[test]
fn ai_prompt_output_writes_file() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    assert!(run(&["ai", "index"], &root).status.success());

    let output = run(
        &[
            "ai",
            "prompt",
            "add new update user endpoint",
            "--output",
            "update-user.prompt.md",
        ],
        &root,
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("AI prompt written to update-user.prompt.md"));
    let prompt = std::fs::read_to_string(root.join("update-user.prompt.md")).unwrap();
    assert!(prompt.contains("# AI implementation prompt"));
    assert!(prompt.contains("route patch \"/users/{id}\""));
    assert!(prompt.contains("handler updateUserRoute"));
}

#[test]
fn ai_prompt_unknown_module_has_low_confidence() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    assert!(run(&["ai", "index"], &root).status.success());

    let output = run(&["ai", "prompt", "add billing endpoint"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("## Confidence\n\nlow"));
    assert!(stdout.contains("No existing module matching \"billing\" was found."));
    assert!(stdout.contains("## Recommended next step"));
}

#[test]
fn ai_prompt_missing_metadata_suggests_ai_index() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    std::fs::remove_file(root.join(".ai/symbols.json")).unwrap();

    let output = run(&["ai", "prompt", "add new update user endpoint"], &root);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("run `vlt ai index`"));
}
