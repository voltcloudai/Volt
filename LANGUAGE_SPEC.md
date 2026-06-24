# Volt Language Spec

This document describes the v0.1 language slice.

## Syntax

Volt uses TypeScript-like syntax with explicit types.

Functions:

```ts
function add(a: i32, b: i32): i32 {
  return a + b
}
```

Variables:

```ts
const name = "Carlos"
let count = 1
const age: i32 = 42
```

Types and object literals:

```ts
type User = {
  id: u64
  email: string
}

const user = User {
  id: 1
  email: "carlos@test.com"
}
```

Conditionals:

```ts
if (value === 0) {
  print("zero")
} else {
  print("nonzero")
}
```

Statements do not require semicolons. Equality is `===` and `!==`; `==` is not part of the language.

## Native HTTP Routes

Routes are top-level declarations. They are first-class language constructs so Volt can parse, check, index, plan, and eventually generate server code from API shape without reverse-engineering framework calls.

```ts
route get "/users/{id}"
  params { id: u64 }
  ok 200 User
  errors {
    UserNotFound 404
    DatabaseError 500
  }
{
  const user = try getUser(params.id, ctx)
  return ok(user)
}
```

Full route shape:

```ts
route patch "/users/{id}"
  params { id: u64 }
  query { verbose: bool }
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

Rules:

- Methods are lowercase: `get`, `post`, `put`, `patch`, `delete`.
- Paths are string literals and path params use `{id}` syntax.
- `params { ... }`, `query { ... }`, `body Type`, `errors { ... }`, and `effects [...]` are optional.
- `ok <status> <Type>` is required.
- Route bodies can refer to implicit `params`, `query`, `body`, and `ctx`.
- GET routes may omit a body and currently produce a diagnostic if they declare one.
- Success statuses must be `100..599`; error statuses must be `400..599`.
- Every `{pathParam}` must be declared in `params`, and every `params` field must appear in the path.

Route declarations lower to Rust/Axum in API builds: path params become `axum::extract::Path`, query params become `axum::extract::Query`, declared bodies become `axum::Json<T>`, state becomes request context, `ok` statuses become `StatusCode` plus JSON responses, and routes are registered on one `axum::Router`.

The current route body lowering subset is intentionally small:

- `return ok(<struct literal>)`
- `return ok(<identifier>)`
- simple `const name = <expr>`
- `params.id`, `query.page`, and `body.email` field access
- string and integer literals
- struct literals
- simple function calls

Unsupported route body syntax produces `EHTTPLOWER001`.

## Supported Types

- `i32`
- `i64`
- `u32`
- `u64`
- `f32`
- `f64`
- `bool`
- `string`
- `void`
- `Result<T, E>`
- User-declared object types with `type Name = { field: Type }`

`string` maps to Rust `String`. `Result<T, E>` maps to Rust `Result<T, E>`.

## Expressions

Supported expressions:

- Integer literals
- Float literals
- String literals
- Boolean literals
- Variable references
- Binary expressions with `+`, `-`, `*`, `/`, `===`, `!==`, `<`, `>`, `<=`, `>=`
- Function calls
- `try` on `Result<T, E>` values in route-oriented code
- Object literals
- Anonymous object literals and spread fields for compact route inputs
- Field access

## Error Model

Volt v0.1 has no exceptions. Fallible operations should use `Result<T, E>`.

Builtins:

```txt
ok(value): Result<T, E>
err(value): Result<T, E>
print(value): void
```

## Memory Model

The memory model is currently inherited from generated Rust. Volt does not expose borrow checking syntax in v0.1. Future versions should define ownership, copying, borrowing, and allocation rules in Volt terms rather than leaking Rust syntax into the language.

## Future AI-Native Tooling

Volt should eventually expose structured compiler diagnostics, stable AST output, and project metadata that AI tools can consume safely. The language should make automated refactoring practical by keeping syntax explicit, types predictable, and compiler passes small.
