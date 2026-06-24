use std::path::{Path, PathBuf};

pub fn create_api_project(base: &Path, name: &str) -> std::io::Result<PathBuf> {
    let root = base.join(name);
    std::fs::create_dir_all(root.join("src/health"))?;
    std::fs::create_dir_all(root.join("src/users"))?;
    std::fs::create_dir_all(root.join(".ai/tasks"))?;
    std::fs::create_dir_all(root.join(".ai/files"))?;

    write(root.join("volt.toml"), &volt_toml(name))?;
    write(root.join("AGENTS.md"), AGENTS)?;
    write(root.join("CLAUDE.md"), CLAUDE)?;
    write(root.join("README.md"), &readme(name))?;
    write(root.join("src/main.vlt"), MAIN)?;
    write(root.join("src/health/health.routes.vlt"), HEALTH_ROUTES)?;
    write(root.join("src/health/health.types.vlt"), HEALTH_TYPES)?;
    write(root.join("src/users/users.routes.vlt"), USERS_ROUTES)?;
    write(root.join("src/users/users.service.vlt"), USERS_SERVICE)?;
    write(
        root.join("src/users/users.repository.vlt"),
        USERS_REPOSITORY,
    )?;
    write(root.join("src/users/users.types.vlt"), USERS_TYPES)?;
    write(root.join("src/users/users.errors.vlt"), USERS_ERRORS)?;
    write(root.join("src/users/users.test.vlt"), USERS_TEST)?;

    write(root.join(".ai/project.md"), &project_md(name))?;
    write(root.join(".ai/architecture.md"), ARCHITECTURE)?;
    write(root.join(".ai/commands.md"), COMMANDS)?;
    write(root.join(".ai/language-rules.md"), LANGUAGE_RULES)?;
    write(root.join(".ai/memory-model.md"), MEMORY_MODEL)?;
    write(root.join(".ai/routes.json"), "[]\n")?;
    write(root.join(".ai/symbols.json"), EMPTY_SYMBOLS)?;
    write(root.join(".ai/source-map.json"), EMPTY_SOURCE_MAP)?;
    write(root.join(".ai/errors.json"), "[]\n")?;
    write(root.join(".ai/dependencies.json"), "{}\n")?;
    write(root.join(".ai/examples.md"), EXAMPLES)?;

    Ok(root)
}

fn write(path: PathBuf, contents: &str) -> std::io::Result<()> {
    std::fs::write(path, contents)
}

fn volt_toml(name: &str) -> String {
    format!(
        r#"[project]
name = "{name}"
type = "api"
entrypoint = "src/main.vlt"
"#
    )
}

fn readme(name: &str) -> String {
    format!(
        r#"# {name}

Generated Volt API project.

## Commands

```sh
vlt ai index
vlt ai summary
vlt build
./target/volt/{name}
curl http://localhost:8080/health
vlt plan "add new update user endpoint"
```

The generated route files use Volt's native `route method "path"` syntax so AI tools can index API shape directly from source.
"#
    )
}

fn project_md(name: &str) -> String {
    format!(
        r#"# Volt Project Summary

Project name: {name}
Entrypoint: src/main.vlt
Project type: api
Modules:
- health
- users
Routes:
- GET /health
- GET /users/{{id}}
Types:
- HealthResponse
- User
- CreateUserInput
Commands:
- Build: `vlt build`
- Format: `vlt fmt src/main.vlt`
- AI index: `vlt ai index`
- Plan task: `vlt plan "<task>"`
"#
    )
}

const AGENTS: &str = r#"# Agent Instructions

- This is a Volt API project.
- Keep syntax TypeScript-like.
- Do not use null, undefined, exceptions, classes, or inheritance.
- Use Result<T, E> for fallible operations.
- Keep route input/output types explicit.
- Prefer native `route method "path"` declarations over `app.get(...)`.
- Keep route bodies inside the supported Axum lowering subset until the compiler grows.
- Run `vlt build` after route changes.
- Run `vlt ai index` after changing source structure.
"#;

const CLAUDE: &str = r#"# Claude Instructions

Use `.ai/project.md`, `.ai/symbols.json`, and `.ai/routes.json` before reading source files. Prefer compact context first, then inspect only relevant modules.

- Prefer native `route method "path"` declarations.
- Keep route bodies inside the supported Axum lowering subset.
- Run `vlt build` after route changes.
"#;

const MAIN: &str = r#"
function healthHandler(): string {
  return "ok"
}

function main(): void {
  print(healthHandler())
}
"#;

const HEALTH_ROUTES: &str = r#"import { HealthResponse } from "./health.types"

route get "/health"
  ok 200 HealthResponse
{
  return ok(HealthResponse {
    status: "ok"
  })
}
"#;

const HEALTH_TYPES: &str = r#"export type HealthResponse = {
  status: string
}
"#;

const USERS_ROUTES: &str = r#"import { User, CreateUserInput } from "./users.types"
import { UserNotFound, DatabaseError, InvalidEmail } from "./users.errors"

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors {
    UserNotFound 404
    DatabaseError 500
  }
{
  return ok(User {
    id: params.id
    email: "demo@test.com"
    name: "Demo User"
  })
}

route post "/users"
  body CreateUserInput
  ok 201 User
  errors {
    InvalidEmail 400
    DatabaseError 500
  }
{
  return ok(User {
    id: 1
    email: body.email
    name: body.name
  })
}
"#;

const USERS_SERVICE: &str = r#"import { User, CreateUserInput } from "./users.types"
import { UserError } from "./users.errors"

export function getUser(id: u64): Result<User, UserError> {
  return ok(User {
    id: id
    email: "demo@test.com"
    name: "Demo User"
  })
}

export function createUser(input: CreateUserInput): Result<User, UserError> {
  return ok(User {
    id: 1
    email: input.email
    name: input.name
  })
}
"#;

const USERS_REPOSITORY: &str = r#"import { User, CreateUserInput } from "./users.types"
import { UserError } from "./users.errors"

export function findUserById(id: u64): Result<User, UserError> {
  return ok(User {
    id: id
    email: "demo@test.com"
    name: "Demo User"
  })
}

export function insertUser(input: CreateUserInput): Result<User, UserError> {
  return ok(User {
    id: 1
    email: input.email
    name: input.name
  })
}
"#;

const USERS_TYPES: &str = r#"export type User = {
  id: u64
  email: string
  name: string
}

export type CreateUserInput = {
  email: string
  name: string
}
"#;

const USERS_ERRORS: &str = r#"export type UserError = {
  code: string
  message: string
}

export type UserNotFound = {
  message: string
}

export type InvalidEmail = {
  message: string
}

export type DatabaseError = {
  message: string
}
"#;
const USERS_TEST: &str = r#"function testCreateUser(): void {
  print("create user test placeholder")
}
"#;

pub const ARCHITECTURE: &str = r#"# Architecture

This project follows route, service, repository, types, errors, and test modules per domain.

- `*.routes.vlt` owns HTTP route handlers.
- `*.service.vlt` owns business logic.
- `*.repository.vlt` owns storage access.
- `*.types.vlt` owns input/output data types.
- `*.errors.vlt` owns explicit Result error types.
"#;

pub const COMMANDS: &str = r#"# Commands

- Check: `vlt check`
- Test: `vlt test`
- Format: `vlt fmt`
- Build: `vlt build`
- AI index: `vlt ai index`
- Plan task: `vlt plan "<task>"`
"#;

pub const LANGUAGE_RULES: &str = r#"# Volt Language Rules for AI Agents

- Use TypeScript-like syntax.
- Do not use null.
- Do not use undefined.
- Do not use exceptions.
- Use Result<T, E> for fallible operations.
- Use Option<T> for optional values.
- Use request ctx.arena for request-scoped allocations.
- Keep route input/output types explicit.
- Use native `route method "path"` declarations for HTTP endpoints.
- Prefer `/users/{id}` path params over `/users/:id`.
- Keep route bodies inside the supported Axum lowering subset: const bindings and `return ok(...)` over literals, field access, calls, and struct literals.
- Run `vlt build` after route changes.
- Run `vlt ai index` after changing source structure.
"#;

pub const MEMORY_MODEL: &str = r#"# Memory Model

Volt uses native memory management.

Initial model:
- Stack values by default.
- Request-scoped allocations through `ctx.arena`.
- Future explicit heap ownership through `box`.
- No garbage collector in the default backend.
"#;

const EMPTY_SYMBOLS: &str = r#"{
  "language": "Volt",
  "entrypoint": "src/main.vlt",
  "types": [],
  "functions": [],
  "routes": [],
  "errors": []
}
"#;

const EMPTY_SOURCE_MAP: &str = r#"{
  "files": []
}
"#;

pub const EXAMPLES: &str = r#"# Examples

```ts
function createUser(input: CreateUserInput): Result<User, CreateUserError> {
  return err("not implemented")
}
```

```ts
route get "/users/{id}"
  params { id: u64 }
  ok 200 User
{
  const user = try getUser(params.id, ctx)
  return ok(user)
}
```
"#;
