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
count = count + 1
const age: i32 = 42
```

`const` bindings are immutable. `let` creates a mutable local variable and assignment is currently statement-only with identifier targets.

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

Boolean operators:

```ts
function canUpdate(isAdmin: bool, isOwner: bool): bool {
  return isAdmin || isOwner
}

function isValid(email: string, name: string): bool {
  return email !== "" && name !== ""
}
```

Arrays:

```ts
function defaultIds(): Array<u64> {
  return [1, 2, 3]
}

function emptyIds(): Array<u64> {
  const ids: Array<u64> = []
  return ids
}
```

Statements do not require semicolons. Equality is `===` and `!==`; `==` is not part of the language. `&&` and `||` require bool operands and do not use JavaScript truthiness.

## Native HTTP Routes

Routes are top-level declarations. They are first-class language constructs so Volt can parse, check, index, plan, and generate server code from API shape without reverse-engineering framework calls. A route is the HTTP contract; normal functions contain business logic.

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
  return getUser(params.id, ctx)
}
```

Full route shape:

```ts
route patch "/users/{id}"
  params { id: u64 }
  query { verbose: bool }
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

function updateUserRoute(
  params: PatchUsersIdParams,
  query: PatchUsersIdQuery,
  body: UpdateUserInput,
  ctx: Ctx
): Result<User, UserError> {
  return updateUser(params.id, query.verbose, body, ctx)
}
```

Rules:

- Methods are lowercase: `get`, `post`, `put`, `patch`, `delete`.
- Paths are string literals and path params use `{id}` syntax.
- `params { ... }`, `query { ... }`, `body Type`, `errors ErrorType { ... }`, legacy `errors { ... }`, `effects [...]`, and `handler name` are optional clauses.
- `ok <status> <Type>` is required.
- A route must define either an inline body or a handler, but not both.
- Handler argument order is `params`, `query`, `body`, `ctx`, omitting undeclared route inputs.
- Generated route params/query type names are stable: `GET /users/{id}` -> `GetUsersIdParams`, `PATCH /users/{id}` -> `PatchUsersIdParams`, `GET /users` with query -> `GetUsersQuery`.
- Inline route bodies can refer to implicit `params`, `query`, `body`, and `ctx`.
- GET routes may omit a body and currently produce a diagnostic if they declare one.
- Success statuses must be `100..599`; error statuses must be `400..599`.
- Every `{pathParam}` must be declared in `params`, and every `params` field must appear in the path.

Route declarations lower to Rust/Axum in API builds: path params become `axum::extract::Path`, query params become `axum::extract::Query`, declared bodies become `axum::Json<T>`, state becomes request context, `ok` statuses become `StatusCode` plus JSON responses, typed `Err(error)` values map through route error status helpers, and routes are registered on one `axum::Router`.

The current inline route body lowering subset is intentionally small:

- `return ok(<struct literal>)`
- `return ok(<identifier>)`
- simple `const name = <expr>`
- `params.id`, `query.page`, and `body.email` field access
- string and integer literals
- struct literals
- simple function calls

Unsupported route body syntax produces `EHTTPLOWER001`. Move business logic into a route handler using `handler myRouteHandler`.

Current limitations: no database integration, middleware/auth, OpenAPI generation, public `match` syntax, or production-ready HTTP runtime yet. Handler lowering is v0.1-level, and only simple generated params/query types are supported.

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
- `Option<T>`
- `Result<T, E>`
- `Array<T>`
- User-declared object types with `type Name = { field: Type }`
- Domain error declarations with `error Name { Variant { field: Type } }`

`string` maps to Rust `String`. `Option<T>` maps to Rust `Option<T>`. `Result<T, E>` maps to Rust `Result<T, E>`. `Array<T>` maps to Rust `Vec<T>`.

## Expressions

Supported expressions:

- Integer literals
- Float literals
- String literals
- Boolean literals
- Variable references
- Binary expressions with `+`, `-`, `*`, `/`, `===`, `!==`, `<`, `>`, `<=`, `>=`, `&&`, `||`
- Function calls
- Call-style struct construction with `User({ id: 1 })`
- Call-style error construction with `UserError.UserNotFound({ message: "..." })`
- `try` on `Result<T, E>` values in route-oriented code
- `none` in `Option<T>` contexts
- Object literals
- Array literals
- Anonymous object literals and spread fields for compact route inputs
- Field access

Array literals are homogeneous. Empty array literals require contextual typing, for example `const ids: Array<u64> = []` or `return []` from a function returning `Array<u64>`.

## Statements

Supported statements include declarations, assignment, return, `if`, `while`, `for-in`, `break`, `continue`, expression statements, and normal TypeScript-like `switch`.

```ts
while (condition) {
  // ...
}

for item in array {
  // item has the Array<T> element type
}

switch (status) {
  case "draft": {
    return "Draft"
  }

  default: {
    return "Unknown"
  }
}
```

`while` conditions follow the same rules as `if`: use bool expressions, or `Option<T>` where Option narrowing is supported. Strings, numbers, and objects are not truthy.

`for-in` currently works over `Array<T>`. The loop variable is scoped to the loop body and is immutable. Generated Rust currently uses direct iteration, so the array is consumed by the loop.

Use `break` and `continue` only inside loops.

`switch` compares ordinary values. It is not pattern matching, it has no fallthrough, and it is not an Option handling strategy. Use `if (value)` / `if (!value)` for `Option<T>`.

## Error Model

Volt v0.1 has no exceptions, `null`, or `undefined`. Absence should use `Option<T>`. Fallible operations should use `Result<T, E>` and domain `error` declarations.

```ts
export error UserError {
  UserNotFound { message: string }
  InvalidEmail { message: string }
}

function maybeUser(id: u64): Option<User> {
  if (id === 1) {
    return User({ id: 1, email: "demo@test.com" })
  }

  return none
}
```

Returning a plain `T` from an `Option<T>` function is accepted and lowered to `Some(T)`. `return none` lowers to `None`.

`if (value)` and `if (!value)` narrow `Option<T>` values. Volt does not implement JavaScript truthiness: strings, numbers, and object types are rejected as conditions unless compared explicitly.

Builtins:

```txt
ok(value): Result<T, E>
err(value): Result<T, E>
print(value): void
```

`switch` is reserved for normal TypeScript-like value control flow. Option handling is done with `if (value)` and `if (!value)` narrowing, not public `match` or `switch (option.kind)` patterns.

## Current Limitations

- No array methods, `.push`, or array indexing yet.
- No field assignment, compound assignment, or increment/decrement yet.
- No JavaScript truthiness.

## Memory Model

The memory model is currently inherited from generated Rust. Volt does not expose borrow checking syntax in v0.1. Future versions should define ownership, copying, borrowing, and allocation rules in Volt terms rather than leaking Rust syntax into the language.

## Future AI-Native Tooling

Volt should eventually expose structured compiler diagnostics, stable AST output, and project metadata that AI tools can consume safely. The language should make automated refactoring practical by keeping syntax explicit, types predictable, and compiler passes small.
