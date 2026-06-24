# Volt

Volt is a native backend language for TypeScript developers. It uses TypeScript-like syntax, rejects JavaScript runtime footguns such as `undefined`, `null`, loose equality, implicit coercion, dynamic prototypes, `any`, and exceptions, and currently compiles to native binaries by generating readable Rust.

The v0.1 goal is a working vertical slice: parse, type-check, generate Rust, build, and run small server-shaped programs.

## Run The Compiler

```sh
cargo run -p vlt_cli -- parse examples/hello.vlt
cargo run -p vlt_cli -- check examples/hello.vlt
cargo run -p vlt_cli -- build examples/hello.vlt
cargo run -p vlt_cli -- run examples/hello.vlt
cargo run -p vlt_cli -- fmt examples/hello.vlt
cargo run -p vlt_cli -- new api my-api
cargo run -p vlt_cli -- ai index
cargo run -p vlt_cli -- ai summary
cargo run -p vlt_cli -- ai prompt "add new update user endpoint"
cargo run -p vlt_cli -- ai prompt "add new update user endpoint" --format codex
cargo run -p vlt_cli -- ai prompt "add new update user endpoint" --format claude
cargo run -p vlt_cli -- ai prompt "add new update user endpoint" --output update-user.prompt.md
cargo run -p vlt_cli -- explain createUser
cargo run -p vlt_cli -- plan "add new update user endpoint"
```

The CLI binary is named `vlt`. Build outputs are written under `target/volt/`.

## Run A Generated API

```sh
vlt new api my-api
cd my-api
vlt build
./target/volt/my-api
curl http://localhost:8080/health
```

Expected response:

```json
{"status":"ok"}
```

`vlt build` in an API project writes a temporary Rust project to `target/volt/rust-project`, builds it with Cargo, and copies the server binary to `target/volt/<project-name>`.

## AI-Native Project Context

Volt projects can generate compact context for AI-assisted engineering:

```sh
vlt ai index
vlt ai summary
vlt ai prompt "add new update user endpoint"
vlt explain createUser
vlt plan "add new update user endpoint"
```

`vlt ai index` scans `.vlt` files with a tolerant indexer and writes:

- `.ai/symbols.json`: project-level types, functions, routes, and errors.
- `.ai/routes.json`: structured native HTTP routes, with legacy route comments and simple `app.get/post/patch/delete(...)` calls as fallback inputs.
- `.ai/source-map.json`: file-level module, kind, symbols, routes, and summaries.
- `.ai/files/*.summary.md`: readable per-file summaries.

The compiler parser remains strict. The AI indexer treats native `route` declarations as the source of truth and remains tolerant of older route comments so existing projects can still produce useful context.

`vlt plan` is offline and deterministic by default. It uses `.ai/` indexes, module conventions, and keyword detection. Future `vlt plan --ai` will be allowed to call an LLM provider, but should send compact `.ai/` context rather than the whole codebase by default.

`vlt ai prompt "<task>"` turns the same offline plan into a paste-ready prompt for AI coding agents. It does not call an LLM. Supported prompt formats:

```sh
vlt ai prompt "add new update user endpoint"
vlt ai prompt "add new update user endpoint" --format codex
vlt ai prompt "add new update user endpoint" --format claude
vlt ai prompt "add new update user endpoint" --output update-user.prompt.md
```

The workflow is:

1. Run `vlt ai index` to refresh compact project metadata.
2. Run `vlt plan "<task>"` to inspect the deterministic implementation plan.
3. Run `vlt ai prompt "<task>"` to generate a prompt that includes the plan, relevant files, language rules, validation commands, and constraints.

## Example

```ts
function add(a: i32, b: i32): i32 {
  return a + b
}

function main(): void {
  const result = add(2, 3)
  print(result)
}
```

Object types are declared with `type` and constructed with TypeScript-like object literals:

```ts
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
```

## Native HTTP Routes

Volt routes are first-class declarations, not framework calls hidden in application code. This keeps backend APIs readable for humans, compact for AI context, easy to index into `.ai/routes.json`, and straightforward to lower into Rust/Axum later.

```ts
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
```

Supported methods are `get`, `post`, `put`, `patch`, and `delete`. Route bodies can refer to implicit `params`, `query`, `body`, and `ctx` symbols. Path params use `{id}` syntax so planner output and generated prompts stay consistent with the language, not Express-style `:id` paths.

Native routes generate structured AI metadata and real Rust/Axum handlers in API builds. Path params become `axum::extract::Path`, query params become `axum::extract::Query`, request bodies become `axum::Json`, success returns use `StatusCode` plus JSON, and routes are registered on one `axum::Router`.

Current route body lowering supports:

- `return ok(<struct literal>)`
- `return ok(<identifier>)`
- simple `const name = <expr>`
- `params.id`, `query.page`, and `body.email` field access
- string and integer literals
- struct literals
- simple function calls already expressible by existing codegen

Unsupported route body syntax fails with `EHTTPLOWER001` instead of generating invalid Rust.

## Current Limitations

- The HTTP runtime is a minimal Axum vertical slice, not a full framework.
- No middleware, authentication, database integration, OpenAPI generation, package manager, or LLVM backend yet.
- No classes, inheritance, decorators, macros, exceptions, `null`, `undefined`, or `any`.
- Generic support is limited to recognizing `Result<T, E>`.
- The formatter is intentionally simple and prints canonical source to stdout.
- Rust code generation is direct and readable, not optimized.
- `vlt plan` is offline and deterministic. It does not call an LLM provider yet.
- `vlt ai prompt` generates prompts only. It does not call Codex, Claude, OpenAI, Anthropic, or any external AI provider.
- Native route lowering only supports a small route body subset.
- Call graph fields are placeholders.
- Generated API projects use native route syntax.

## Roadmap

1. Improve diagnostics and add recovery for more parser errors.
2. Expand API project support around routes, services, repositories, and tests.
3. Add assignment, loops, arrays, and `Option<T>`.
4. Add richer formatter behavior and snapshot tests.
5. Add backend-focused standard library pieces.
6. Explore provider-backed `vlt plan --ai` using compact `.ai/` context.
7. Explore direct native backends after the language core stabilizes.
