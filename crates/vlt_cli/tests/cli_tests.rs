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
    assert!(symbols.contains("\"name\": \"createUser\""));
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
    assert!(generated.contains("axum::Router"));
    assert!(generated.contains("tokio::main"));
    assert!(generated.contains("axum::serve"));
    assert!(generated.contains(".route(\"/health\", get(get_health_route))"));
    assert!(generated.contains("async fn get_health_route"));
    assert!(generated.contains("async fn get_users_id_route"));
    assert!(generated.contains("async fn post_users_route"));

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
    let response = request_health(port, &mut child);
    assert!(response.contains("200 OK"), "{response}");
    assert!(response.contains(r#"{"status":"ok"}"#), "{response}");
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn unsupported_route_body_fails_before_rust_is_generated() {
    let dir = tempdir().unwrap();
    assert!(run(&["new", "api", "my-api"], dir.path()).status.success());
    let root = dir.path().join("my-api");
    std::fs::write(
        root.join("src/health/health.routes.vlt"),
        r#"route get "/health"
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

fn request_health(port: u16, child: &mut Child) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
            stream
                .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .unwrap();
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

    let output = run(&["explain", "createUser"], &root);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# Symbol: createUser"));
    assert!(stdout.contains("function createUser"));
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
    assert!(stdout.contains("PATCH /users/{id}"));
    assert!(stdout.contains("Confidence:\nhigh"));
    assert!(stdout.contains("Inferred intent:\nUpdate an existing user."));
    assert!(stdout.contains("src/users/users.routes.vlt"));
    assert!(stdout.contains("src/users/users.service.vlt"));
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
    assert!(routes.contains("\"name\": \"InvalidEmail\""));
    assert!(routes.contains("\"status\": 400"));
    assert!(routes.contains("\"effects\""));
    assert!(routes.contains("\"db\""));
    assert!(routes.contains("\"alloc\""));
    assert!(routes.contains("\"log\""));

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
    assert!(stdout.contains("PATCH /users/{id}"));
    assert!(stdout.contains("src/users/users.routes.vlt"));
    assert!(stdout.contains("src/users/users.service.vlt"));
    assert!(stdout.contains("## Validation commands"));
    assert!(stdout.contains("vlt ai index"));
    assert!(stdout.contains("vlt build"));
    assert!(stdout.contains("Use Result<T, E> for fallible operations."));
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
    assert!(prompt.contains("PATCH /users/{id}"));
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
