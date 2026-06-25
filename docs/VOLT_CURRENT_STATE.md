# Volt Current State

This document describes the currently implemented Volt language slice.
For the long-term production direction, see docs/VOLT_1_0_VISION.md.

This document should be updated after every language phase. Codex, Claude, and other AI agents should use this file as the source of truth for currently supported Volt syntax.

## Current Version Slice

Volt currently has:

- A Rust compiler workspace.
- Parser, checker, formatter, and Rust codegen.
- Import/export support and a multi-file resolver.
- Native HTTP route declarations.
- External route handlers.
- `Result<T, E>`.
- `Option<T>`.
- `none`.
- `if (option)` and `if (!option)` narrowing.
- Domain error declarations.
- Typed route errors.
- Axum wrappers for generated API projects.
- `vlt new api`.
- `vlt ai index`, `vlt ai summary`, `vlt explain`, `vlt plan`, and `vlt ai prompt`.

## Implemented Syntax

The current language slice supports:

- Functions.
- Type declarations.
- Error declarations.
- `const`.
- `let`.
- Assignment to `let` bindings.
- Field assignment on mutable struct locals.
- Array literals.
- `Array<T>`.
- `array.push(value)` on mutable arrays.
- Array indexing.
- `if` / `else`.
- `while`.
- `for item in array`.
- `break`.
- `continue`.
- `switch`.
- `return`.
- Function calls.
- Struct literals.
- Object literals where currently supported.
- Route declarations.
- Handler declarations.

## Current Memory Model

Volt uses deterministic memory management through its Rust backend.

Volt does not expose Rust lifetimes, references or borrow checker syntax.

Volt programs use owned values at the language level.

Bindings:

- `const` creates an immutable binding.
- `let` creates a mutable binding.

Values:

- Scalars may be copied.
- Strings, arrays and structs are owned values.
- The compiler may clone owned values when needed to preserve simple Volt semantics.
- Generated structs derive Clone.

Arrays:

- `Array<T>` lowers to `Vec<T>`.
- Array indexing returns an owned value.
- For non-Copy values, generated Rust may clone.
- Out-of-bounds indexing may panic in the current version.

Mutation:

- Only `let` bindings can be mutated.
- Field assignment requires a mutable local struct.
- `array.push(value)` requires a mutable local array.
- Loop variables remain immutable.

Implementation detail:

- Volt may use request-scoped allocation or arenas internally in the future.
- This is not exposed as `ctx.arena` or any manual allocation API in Volt code.

## Current Type System

Supported types:

- `i32`.
- `i64`.
- `u32`.
- `u64`.
- `f32`.
- `f64`.
- `bool`.
- `string`.
- `void`.
- `Option<T>`.
- `Result<T, E>`.
- `Array<T>`.
- User-declared object types.
- Domain error declarations.

## Current HTTP Support

Supported route methods are `get`, `post`, `put`, `patch`, and `delete`.

Route declarations support:

- `params { ... }`.
- `query { ... }`.
- `body Type`.
- `ok <status> <Type>`.
- `errors DomainError { Variant 404 }`.
- Legacy `errors { Variant 404 }`.
- `effects [db, log]`.
- `handler handlerName`.
- Tiny inline route bodies.

Path params use `{id}` syntax. Handler argument order is `params`, `query`, `body`, `ctx`, omitting inputs the route does not declare. Generated route params and query names are stable, for example `GetUsersIdParams`, `PatchUsersIdParams`, and `GetUsersQuery`.

Typed route error values map through route error status helpers. API builds lower supported routes to Axum wrappers. Inline route lowering is intentionally small and should be used only for tiny endpoints such as `/health`.

## Current AI Tooling

Current AI commands:

- `vlt ai index`.
- `vlt ai summary`.
- `vlt explain`.
- `vlt plan`.
- `vlt ai prompt`.

Prompt and plan generation are deterministic and offline. There is no real LLM integration yet.

`vlt ai index` writes compact project metadata under `.ai/`, including symbols, routes, source maps, summaries, language rules, memory model notes, and examples.

## Current Limitations

- No database integration.
- No middleware or auth.
- No OpenAPI generation.
- No package manager.
- No public `match`.
- No JavaScript truthiness.
- No classes.
- No decorators.
- No `any`.
- No `null`.
- No `undefined`.
- No `async` / `await`.
- No advanced generics beyond recognized `Option<T>`, `Result<T, E>`, and `Array<T>`.
- No array `map`, `filter`, or `find`.
- No object spread updates beyond existing object literal support.
- No public arena API.
- No public borrow, lifetime, or reference syntax.
- No public Box, Rc, Arc, unsafe, raw pointer, or manual allocation APIs.
- Inline Axum lowering supports only a small route body subset.
- Rust codegen prioritizes readable correctness over optimization.

## Implemented Examples

Owned struct mutation:

```volt
type User = {
  id: u64
  email: string
}

function updateEmail(user: User): User {
  let editable = user
  editable.email = "new@test.com"
  return editable
}
```

Array push:

```volt
function ids(): Array<u64> {
  let values: Array<u64> = []
  values.push(1)
  values.push(2)
  return values
}
```

Array indexing:

```volt
function firstEmail(users: Array<User>): string {
  const first = users[0]
  return first.email
}
```

Route handler:

```volt
route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors UserError {
    UserNotFound 404
    DatabaseError 500
  }
  handler getUserRoute

function getUserRoute(params: GetUsersIdParams, ctx: Ctx): Result<User, UserError> {
  return getUser(params.id, ctx)
}
```

Result and Option:

```volt
function maybeUser(id: u64): Result<Option<User>, UserError> {
  if (id === 1) {
    return ok(User({
      id: 1
      email: "demo@test.com"
    }))
  }

  return ok(none)
}
```

## Gap To Volt 1.0

| Area | Current State | Volt 1.0 Direction | Status |
| --- | --- | --- | --- |
| Memory model | Owned values, Clone-based generated Rust, no public Rust memory syntax | Deterministic owned-value semantics with better optimization | In progress |
| HTTP routes | Native route declarations and Axum vertical slice | Production API runtime with explicit contracts | In progress |
| AI metadata | Index, summary, explain, plan, prompt | Rich compact metadata for agents and tools | In progress |
| DB integration | Not implemented | Repository/storage boundaries with optional integrations | Future |
| Middleware/auth | Not implemented | First-class backend capability | Future |
| OpenAPI | Not implemented | Generated API descriptions from route contracts | Future |
| Package manager | Not implemented | Project dependency workflow | Future |
| Optimizer | Readable Rust, may clone | Fewer clones and better generated Rust | Future |
| LSP | Not implemented | Editor diagnostics and navigation | Future |
| Safe indexing | Indexing may panic | Option-returning safe access or equivalent | Future |
| Deployment | Local build/run only | Deployment templates and runtime packaging | Future |
