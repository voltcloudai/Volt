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
    write(root.join(".ai/current-language.md"), CURRENT_LANGUAGE)?;
    write(root.join(".ai/volt-1-0-direction.md"), VOLT_1_0_DIRECTION)?;
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

The generated route files use Volt's native `route method "path"` syntax as HTTP contracts. Business logic lives in handler functions so AI tools can index API shape directly from source.

AI agents should read `.ai/current-language.md`, `.ai/language-rules.md`, and `.ai/memory-model.md` before writing Volt code. `.ai/volt-1-0-direction.md` is long-term direction only.
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
- POST /users
Types:
- HealthResponse
- User
- CreateUserInput
Commands:
- Build: `vlt build`
- Format: `vlt fmt src/main.vlt`
- AI index: `vlt ai index`
- Plan task: `vlt plan "<task>"`
Language:
- Current source of truth: `.ai/current-language.md`
- Direction only: `.ai/volt-1-0-direction.md`
"#
    )
}

const AGENTS: &str = r#"# Agent Instructions

- This is a Volt API project.
- Use `.ai/current-language.md` as the source of truth for currently supported Volt syntax.
- Use `.ai/volt-1-0-direction.md` only for long-term direction.
- Do not implement or use a future feature just because it appears in the 1.0 direction document.
- Keep syntax TypeScript-like.
- Do not use null, undefined, exceptions, classes, or inheritance.
- Use Result<T, E> for fallible operations.
- Use Option<T> for absence; prefer `return none`.
- Use owned-value semantics.
- Do not expose Rust references, lifetimes, borrow annotations, Box, Rc, Arc, unsafe, raw pointers, or manual memory APIs in Volt code.
- Do not use `ctx.arena` in user Volt code; request-scoped arenas are internal-only future implementation details.
- Generated structs derive Clone.
- Use `let` for mutable local variables.
- Use assignment only on `let` bindings; `const` bindings are immutable.
- Use `&&` and `||` only with bool operands.
- Use `Array<T>` for arrays; empty arrays require contextual type, for example `const ids: Array<u64> = []`.
- Use `array[index]` to read array values.
- `array[index]` may panic at runtime if out of bounds in current codegen.
- Use `array.length` for array length; it returns `u64`.
- Use `let array: Array<T> = []` before calling `array.push(value)` on an empty array.
- Use `array.push(value)` only on mutable arrays.
- Array indexing returns an owned value.
- Use field assignment only on mutable local structs.
- Do not use nested field assignment, field assignment on Option-narrowed bindings, or array element assignment.
- Use `+=` and `-=` for numeric compound assignment.
- Use `++` and `--` only as standalone statements on mutable numeric variables.
- Do not use prefix `++` / `--`, field compound assignment, or `*=`, `/=`, `%=`.
- Assignment remains statement-only.
- Use `while (condition) { ... }` for condition-based loops.
- Use `for item in array { ... }` to iterate over `Array<T>`.
- Use `break` and `continue` only inside loops.
- Use normal TypeScript-like `switch` for ordinary value branching; it is not pattern matching and has no fallthrough.
- Do not use `switch` for Option<T>; use `if (value)` / `if (!value)`.
- Use domain `error` declarations for typed errors.
- Keep route input/output types explicit.
- Prefer native `route method "path"` declarations over `app.get(...)`.
- Treat route declarations as HTTP contracts and put business logic in handler functions.
- Use handler argument order: params, query, body, ctx.
- Keep inline route bodies tiny and inside the supported Axum lowering subset until the compiler grows.
- Current limits: no DB integration, middleware/auth, OpenAPI, public match, or JS truthiness.
- Run `vlt build` after route changes.
- Run `vlt ai index` after changing source structure.
"#;

const CLAUDE: &str = r#"# Claude Instructions

Use `.ai/current-language.md` as the source of truth for currently supported Volt syntax.
Use `.ai/volt-1-0-direction.md` only for long-term direction.
Do not implement or use a future feature just because it appears in the 1.0 direction document.

Use `.ai/project.md`, `.ai/symbols.json`, and `.ai/routes.json` before reading source files. Prefer compact context first, then inspect only relevant modules.

- Prefer native `route method "path"` declarations.
- Treat route declarations as HTTP contracts and put business logic in handler functions.
- Use handler argument order: params, query, body, ctx.
- Use owned-value semantics.
- Do not expose Rust references, lifetimes, borrow annotations, Box, Rc, Arc, unsafe, raw pointers, or manual memory APIs in Volt code.
- Do not use `ctx.arena` in user Volt code; request-scoped arenas are internal-only future implementation details.
- Generated structs derive Clone.
- Use `let` for mutable local variables and assign only to `let` bindings.
- Use `Array<T>` and `[a, b, c]` for arrays; annotate empty arrays.
- Use `array[index]` to read array values.
- `array[index]` may panic at runtime if out of bounds in current codegen.
- Use `array.length` for array length; it returns `u64`.
- Use `let array: Array<T> = []` before calling `array.push(value)` on an empty array.
- Use `array.push(value)` only on mutable arrays.
- Array indexing returns an owned value.
- Use field assignment only on mutable local structs.
- Do not use nested field assignment, field assignment on Option-narrowed bindings, or array element assignment.
- Use `+=` and `-=` for numeric compound assignment.
- Use `++` and `--` only as standalone statements on mutable numeric variables.
- Do not use prefix `++` / `--`, field compound assignment, or `*=`, `/=`, `%=`.
- Assignment remains statement-only.
- Use `while (condition) { ... }`, `for item in array { ... }`, and normal TypeScript-like `switch` for value branching.
- Use `break` and `continue` only inside loops; do not use `switch` for Option<T>`.
- Keep inline route bodies tiny and inside the supported Axum lowering subset.
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
  return ok(HealthResponse({
    status: "ok"
  }))
}
"#;

const HEALTH_TYPES: &str = r#"export type HealthResponse = {
  status: string
}
"#;

const USERS_ROUTES: &str = r#"import { User, CreateUserInput } from "./users.types"
import { UserError } from "./users.errors"
import { getUserRoute, createUserRoute } from "./users.service"

route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors UserError {
    UserNotFound 404
    DatabaseError 500
  }
  effects [db, log]
  handler getUserRoute

route post "/users"
  body CreateUserInput
  ok 201 User
  errors UserError {
    InvalidEmail 400
    DatabaseError 500
  }
  effects [db, log]
  handler createUserRoute
"#;

const USERS_SERVICE: &str = r#"import { User, CreateUserInput } from "./users.types"
import { UserError } from "./users.errors"

export function getUserRoute(
  params: GetUsersIdParams,
  ctx: Ctx
): Result<User, UserError> {
  return ok(User({
    id: params.id
    email: "demo@test.com"
    name: "Demo User"
  }))
}

export function createUserRoute(
  body: CreateUserInput,
  ctx: Ctx
): Result<User, UserError> {
  return ok(User({
    id: 1
    email: body.email
    name: body.name
  }))
}
"#;

const USERS_REPOSITORY: &str = r#"import { User, CreateUserInput } from "./users.types"
import { UserError } from "./users.errors"

export function findUserById(id: u64): Result<User, UserError> {
  return ok(User({
    id: id
    email: "demo@test.com"
    name: "Demo User"
  }))
}

export function insertUser(input: CreateUserInput): Result<User, UserError> {
  return ok(User({
    id: 1
    email: input.email
    name: input.name
  }))
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

const USERS_ERRORS: &str = r#"export error UserError {
  UserNotFound { message: string }
  InvalidEmail { message: string }
  EmailAlreadyExists { email: string }
  DatabaseError { message: string }
}
"#;
const USERS_TEST: &str = r#"function testCreateUser(): void {
  print("create user test placeholder")
}
"#;

pub const ARCHITECTURE: &str = r#"# Architecture

This project follows route, service, repository, types, errors, and test modules per domain.

- `*.routes.vlt` owns HTTP contracts.
- `*.service.vlt` owns business logic and route handler functions.
- `*.repository.vlt` owns storage access.
- `*.types.vlt` owns input/output data types.
- `*.errors.vlt` owns explicit Result error types.
- `*.test.vlt` files are placeholders until the native Phase 5 test runner exists.
"#;

pub const COMMANDS: &str = r#"# Commands

- Check: `vlt check`
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
- Use Option<T> for absence.
- Prefer `return none` for absent Option<T> values.
- Prefer returning the plain value from Option<T> functions; the compiler wraps it.
- Use `if (value)` and `if (!value)` to narrow Option<T>.
- Do not use truthiness for strings, numbers, or objects; use explicit comparisons.
- Use owned values.
- Generated structs derive Clone.
- Do not use Rust references, lifetimes, borrow annotations, Box, Rc, Arc, unsafe, raw pointers, or manual memory APIs in Volt code.
- Do not use `ctx.arena` in user Volt code.
- Volt may use request-scoped allocation or arenas internally in the future, but this is an implementation detail and is not exposed as `ctx.arena` or a manual allocation API in Volt code.
- Use `let` for mutable local variables.
- Use assignment only on `let` bindings.
- `const` bindings are immutable.
- Use `&&` and `||` only with bool operands.
- Volt does not use JavaScript truthiness.
- Use `Array<T>` for arrays.
- Array literals use `[a, b, c]`.
- Empty arrays require contextual type, e.g. `const ids: Array<u64> = []`.
- Use `array[index]` to read array values.
- `array[index]` may panic at runtime if out of bounds in current codegen.
- Use `array.length` for array length.
- `array.length` returns `u64`.
- Use `let array: Array<T> = []` before calling `array.push(value)` on an empty array.
- Use `array.push(value)` only on mutable arrays.
- Array indexing returns an owned value.
- For non-Copy values, array indexing may clone in generated Rust.
- Use field assignment only on mutable local structs.
- Do not use nested field assignment.
- Do not use field assignment on Option-narrowed bindings.
- Do not use array element assignment.
- Use `+=` and `-=` for numeric compound assignment.
- Use `++` and `--` only as standalone postfix statements on mutable numeric variables.
- Do not use prefix `++` or `--`.
- Do not use `*=`, `/=`, or `%=`.
- Assignment remains statement-only.
- Use `while (condition) { ... }` for condition-based loops.
- Use `for item in array { ... }` to iterate over `Array<T>`.
- `for-in` currently works over `Array<T>`.
- `for-in` currently consumes the array in generated Rust.
- Use `break` and `continue` only inside loops.
- Use normal TypeScript-like `switch` for ordinary value branching.
- `switch` is not pattern matching.
- Do not use `switch` for Option<T>; use `if (value)` / `if (!value)`.
- No fallthrough in switch.
- Use domain `error` declarations for typed errors.
- Prefer typed route errors with `errors DomainError { Variant 404 }`.
- Keep route input/output types explicit.
- Use native `route method "path"` declarations for HTTP endpoints.
- Prefer `/users/{id}` path params over `/users/:id`.
- Use handler argument order: params, query, body, ctx.
- Generated route input type names are stable, for example `GetUsersIdParams`, `PatchUsersIdParams`, and `GetUsersQuery`.
- `Ok(value)` becomes the route success status plus JSON; `Err(error)` maps through typed route errors to an HTTP status plus JSON.
- Keep inline route bodies tiny and inside the supported Axum lowering subset: const bindings and `return ok(...)` over literals, field access, calls, and struct literals.
- Current limits: no DB integration, middleware/auth, OpenAPI, public match, production-ready HTTP framework, or JavaScript truthiness.
- Run `vlt build` after route changes.
- Run `vlt ai index` after changing source structure.
"#;

pub const MEMORY_MODEL: &str = r#"# Memory Model

Volt uses deterministic memory management through its Rust backend.

Volt programs use owned values at the language level.

Bindings:
- `const` creates an immutable binding.
- `let` creates a mutable binding.

Values:
- Scalars may be copied.
- Strings, arrays, and structs are owned values.
- Generated structs derive Clone.
- The compiler may clone owned values when needed to preserve simple Volt semantics.

Arrays:
- `Array<T>` lowers to `Vec<T>`.
- Array indexing returns an owned value.
- For non-Copy values, generated Rust may clone.
- `array.length` returns `u64`.
- Out-of-bounds indexing may panic in the current version.
- `array.push(value)` requires a mutable local array.

Mutation:
- Only `let` bindings can be mutated.
- Field assignment requires a mutable local struct.
- Numeric `+=`, `-=`, `++`, and `--` require mutable local numeric variables.
- Loop variables remain immutable.

Implementation detail:
- Volt may use request-scoped allocation or arenas internally in the future, but this is an implementation detail and is not exposed as `ctx.arena` or a manual allocation API in Volt code.
- Do not use Rust references, lifetimes, borrow annotations, Box, Rc, Arc, unsafe, raw pointers, or manual memory APIs in Volt code.
"#;

pub const CURRENT_LANGUAGE: &str = r#"# Current Volt Language

Use this file as the source of truth for currently supported syntax in this project.

- Volt syntax is TypeScript-like.
- Routes use native `route method "path"` declarations with `{id}` path params.
- Use `Option<T>` and `none` for absence.
- Use `Result<T, E>` and domain `error` declarations for fallible operations.
- Use `if (value)` and `if (!value)` to narrow `Option<T>`.
- Do not use JavaScript truthiness for strings, numbers, or objects.
- Use owned values.
- Generated structs derive Clone.
- Do not use `ctx.arena`; arenas are internal-only future implementation details.
- Do not use Rust references, lifetimes, Box, Rc, Arc, unsafe, raw pointers, or manual memory APIs in Volt code.
- `const` bindings are immutable.
- `let` bindings are mutable.
- Assignment is only valid on mutable locals.
- Field assignment requires a mutable local struct.
- `Array<T>` lowers to `Vec<T>`.
- Use `array[index]` to read array values.
- `array.length` returns `u64`.
- `array.push(value)` requires a mutable local array.
- Array indexing returns an owned value and may clone for non-Copy values.
- Use `+=` and `-=` for numeric compound assignment.
- Use `++` and `--` only as standalone statements on mutable numeric variables.
- Use `while`, `for item in array`, `break`, `continue`, and normal TypeScript-like `switch`.
- Do not use `switch` for `Option<T>`.
- Keep inline route bodies tiny; put business logic in handler functions.
"#;

pub const VOLT_1_0_DIRECTION: &str = r#"# Volt 1.0 Direction

This file is direction only. Do not implement or use a future feature just because it appears here.

Volt 1.0 aims to be an AI-native backend programming language for building fast APIs with TypeScript-like ergonomics and Rust-backed performance for backend workloads.

Direction:
- Backend-first and API-first.
- Route, service, repository, types, errors, and tests per domain.
- Explicit DTOs and typed route errors.
- Owned-value language semantics with Rust memory details hidden from users.
- No JavaScript runtime.
- No exposed garbage collector.
- No public Rust lifetimes, references, borrow checker syntax, Box, Rc, Arc, unsafe, raw pointers, manual allocation APIs, or `ctx.arena`.
- Future features may include middleware/auth, database integrations, OpenAPI, package management, safe indexing, richer arrays, optimizer passes, internal arena optimization, incremental compilation, and LSP support.
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
  return err(CreateUserError.DatabaseError({ message: "not implemented" }))
}
```

```ts
type User = {
  id: u64
  email: string
}

function demoOwnedMutation(): string {
  let users: Array<User> = []
  let user = User({
    id: 1
    email: "old@test.com"
  })

  user.email = "new@test.com"
  users.push(user)

  const first = users[0]
  return first.email
}
```

```ts
route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors UserError {
    UserNotFound 404
    DatabaseError 500
  }
  handler getUserRoute

function getUserRoute(params: GetUsersIdParams, ctx: Ctx): Result<User, UserError> {
  return err(UserError.UserNotFound({ message: "not implemented" }))
}
```
"#;
