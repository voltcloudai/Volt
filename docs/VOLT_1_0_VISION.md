# Volt 1.0 Vision

This document describes the intended production direction for Volt 1.0.
It is not a list of currently implemented features.
For currently supported syntax and compiler behavior, see docs/VOLT_CURRENT_STATE.md and LANGUAGE_SPEC.md.

## Product Goal

Volt is an AI-native backend programming language for building fast APIs with TypeScript-like ergonomics and Rust-backed performance for backend workloads.

Volt is backend-first and API-first. It should make services, handlers, DTOs, domain errors, and route contracts easy for humans and AI agents to read. The language should be simple to parse, straightforward to type-check, and natural to compile into readable Rust.

Volt does not use a JavaScript runtime. It does not expose a garbage collector to users. It does not expose Rust lifetimes, references, borrow checker syntax, Box, Rc, Arc, unsafe, raw pointers, or manual memory APIs in Volt code.

## Design Principles

- TypeScript-like readability.
- Rust-grade performance through generated Rust.
- Explicit types at API and domain boundaries.
- No JavaScript truthiness.
- No `null` or `undefined`.
- No `any`.
- No classes or decorators in the core language.
- No user-visible lifetimes or references.
- Predictable control flow.
- Small, composable compiler passes.
- AI-native project structure and metadata.
- Backend APIs as first-class language constructs.

## Memory Model

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

Early versions should prefer Clone-based correctness over complicated user-visible memory rules. Future optimizer passes may reduce unnecessary clones, reuse storage, or introduce zero-copy codegen where safe. Zero-copy is an internal optimization goal, not a user-facing memory model.

## Type System

Volt 1.0 should support:

- Scalar integer and float types.
- `string`.
- `bool`.
- `void`.
- `Array<T>`.
- `Option<T>`.
- `Result<T, E>`.
- Domain error declarations.
- User-defined object types.
- Explicit route input and output types.
- Simple generics where they serve backend APIs and reusable domain helpers.

Advanced generics are future work, not current syntax. Volt should not add `any`, `null`, or `undefined`.

## Error Model

Volt should not use exceptions for ordinary failures. Absence should use `Option<T>`. Fallible operations should use `Result<T, E>`. Domain errors should be declared with `error` declarations and constructed explicitly, for example `UserError.UserNotFound({ message: "..." })`.

Routes should declare typed error mappings so HTTP status behavior is visible in the route contract.

## API Model

Routes are first-class declarations. Route files own HTTP contracts. Service files own business logic. Repository files own storage boundaries. Types files own DTOs. Errors files own domain errors. Handler functions connect routes to business logic. API builds generate a Rust/Axum runtime.

```volt
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
```

## Example Production-Style Module

This example is production-style Volt 1.0 direction. Check docs/VOLT_CURRENT_STATE.md before using every construct in today's compiler.

```volt
type User = {
  id: u64
  email: string
  name: string
}

type UpdateUserInput = {
  email: Option<string>
  name: Option<string>
}

error UserError {
  UserNotFound { message: string }
  InvalidEmail { message: string }
  EmailAlreadyExists { email: string }
  DatabaseError { message: string }
}

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

function updateUserRoute(
  params: PatchUsersIdParams,
  body: UpdateUserInput,
  ctx: Ctx
): Result<User, UserError> {
  return updateUser(params.id, body, ctx)
}

function updateUser(
  id: u64,
  input: UpdateUserInput,
  ctx: Ctx
): Result<User, UserError> {
  const existing = try findUserById(id, ctx)

  if (!existing) {
    return err(UserError.UserNotFound({ message: "user not found" }))
  }

  let user = existing

  if (input.email) {
    user.email = input.email
  }

  if (input.name) {
    user.name = input.name
  }

  let auditEmails: Array<string> = []
  auditEmails.push(user.email)
  const firstAuditEmail = auditEmails[0]

  return saveUser(user, firstAuditEmail, ctx)
}

function findUserById(id: u64, ctx: Ctx): Result<Option<User>, UserError> {
  if (id === 1) {
    return ok(User({
      id: 1
      email: "demo@test.com"
      name: "Demo User"
    }))
  }

  return ok(none)
}

function saveUser(user: User, auditEmail: string, ctx: Ctx): Result<User, UserError> {
  return ok(user)
}
```

## Compilation Model

Volt parses and checks source files, resolves imports and exports across modules, and emits Rust. API builds emit Axum server projects. Generated Rust may use Clone for correctness. Future optimization passes may reduce unnecessary clones.

Rust details should not leak into Volt user syntax. Users write Volt values, routes, errors, handlers, and DTOs; the compiler owns Rust lowering details.

## AI-Native Model

Volt projects should expose compact metadata for AI-assisted engineering:

- `.ai/project.md`.
- `.ai/symbols.json`.
- `.ai/routes.json`.
- `.ai/source-map.json`.
- `.ai/language-rules.md`.
- `.ai/memory-model.md`.
- `vlt ai index`.
- `vlt ai summary`.
- `vlt explain`.
- `vlt plan`.
- `vlt ai prompt`.

AI agents should read compact metadata first, then inspect only relevant files. Generated prompts should distinguish current syntax from long-term direction so agents do not accidentally use future features.

## Reserved / Future Features

These are possible future features and are not current unless docs/VOLT_CURRENT_STATE.md says they are implemented:

- Middleware and auth.
- Database integrations.
- OpenAPI generation.
- Package manager.
- Async runtime abstractions.
- Safe indexing.
- Richer array helpers.
- Migrations.
- Validation DSL.
- Deployment templates.
- Better optimizer.
- Internal arena optimization.
- Incremental compilation.
- LSP.

## Non-Goals

Volt 1.0 is not:

- A JavaScript runtime.
- A TypeScript superset.
- A general systems programming language.
- A Rust replacement.
- A frontend language.
- A language exposing manual memory management.
- A language exposing the borrow checker.
