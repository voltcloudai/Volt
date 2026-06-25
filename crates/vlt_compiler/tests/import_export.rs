use std::fs;
use tempfile::tempdir;
use vlt_compiler::resolve_project;

#[test]
fn resolves_exported_type_import() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("volt.toml"),
        r#"[project]
name = "test"
type = "api"
entrypoint = "src/main.vlt"
"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("src/users")).unwrap();

    fs::write(
        root.join("src/users/users.types.vlt"),
        r#"export type User = {
  id: u64
  email: string
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("src/users/users.routes.vlt"),
        r#"import { User } from "./users.types"

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
{
  return ok(User {
    id: params.id
    email: "demo@test.com"
  })
}
"#,
    )
    .unwrap();

    let resolved = resolve_project(root).unwrap();

    assert!(resolved
        .program
        .declarations
        .iter()
        .any(|decl| matches!(decl, vlt_compiler::ast::Decl::Route(_))));
}

#[test]
fn fails_when_importing_non_exported_symbol() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("volt.toml"),
        r#"[project]
name = "test"
type = "api"
entrypoint = "src/main.vlt"
"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("src/users")).unwrap();

    fs::write(
        root.join("src/users/users.types.vlt"),
        r#"type User = {
  id: u64
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("src/users/users.routes.vlt"),
        r#"import { User } from "./users.types"

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
{
  return ok(User {
    id: params.id
  })
}
"#,
    )
    .unwrap();

    let err = resolve_project(root).unwrap_err();
    let message = err.to_string();

    assert!(
        message.contains("compiler diagnostics"),
        "unexpected error: {message}"
    );
}

#[test]
fn fails_when_symbol_is_used_without_import() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("volt.toml"),
        r#"[project]
name = "test"
type = "api"
entrypoint = "src/main.vlt"
"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("src/users")).unwrap();

    fs::write(
        root.join("src/users/users.types.vlt"),
        r#"export type User = {
  id: u64
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("src/users/users.routes.vlt"),
        r#"route get "/users/{id}"
  params { id: u64 }
  ok 200 User
{
  return ok(User {
    id: params.id
  })
}
"#,
    )
    .unwrap();

    let err = resolve_project(root).unwrap_err();
    let message = err.to_string();

    assert!(
        message.contains("compiler diagnostics"),
        "unexpected error: {message}"
    );
}

#[test]
fn resolves_exported_error_import() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("volt.toml"),
        r#"[project]
name = "test"
type = "api"
entrypoint = "src/main.vlt"
"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("src/users")).unwrap();

    fs::write(
        root.join("src/users/users.errors.vlt"),
        r#"export error UserError {
  UserNotFound { message: string }
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("src/users/users.types.vlt"),
        r#"export type User = {
  id: u64
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("src/users/users.service.vlt"),
        r#"import { User } from "./users.types"
import { UserError } from "./users.errors"

export function getUser(): Result<User, UserError> {
  return err(UserError.UserNotFound({ message: "missing" }))
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("src/users/users.routes.vlt"),
        r#"import { User } from "./users.types"
import { UserError } from "./users.errors"

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
    )
    .unwrap();

    let resolved = resolve_project(root).unwrap();

    assert!(resolved
        .program
        .declarations
        .iter()
        .any(|decl| matches!(decl, vlt_compiler::ast::Decl::Error(_))));
}

#[test]
fn fails_when_importing_non_exported_error() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("volt.toml"),
        r#"[project]
name = "test"
type = "api"
entrypoint = "src/main.vlt"
"#,
    )
    .unwrap();

    fs::create_dir_all(root.join("src/users")).unwrap();

    fs::write(
        root.join("src/users/users.errors.vlt"),
        r#"error UserError {
  UserNotFound { message: string }
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("src/users/users.routes.vlt"),
        r#"import { UserError } from "./users.errors"

type User = { id: u64 }

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
    )
    .unwrap();

    let err = resolve_project(root).unwrap_err();
    let message = err.to_string();

    assert!(
        message.contains("compiler diagnostics"),
        "unexpected error: {message}"
    );
}
