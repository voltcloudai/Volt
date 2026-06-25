# AI Integration Design

Volt's AI workflow is deterministic first.

Related language docs:

- [Current supported syntax and compiler behavior](VOLT_CURRENT_STATE.md)
- [Volt 1.0 vision and long-term direction](VOLT_1_0_VISION.md)

AI agents should use `docs/VOLT_CURRENT_STATE.md` as the source of truth for currently supported Volt syntax. `docs/VOLT_1_0_VISION.md` is direction only; do not implement or use a future feature just because it appears there.

```sh
vlt plan "add update user endpoint"
vlt ai prompt "add update user endpoint"
vlt ai prompt "add update user endpoint" --format codex
vlt ai prompt "add update user endpoint" --format claude
vlt ai prompt "add update user endpoint" --output update-user.prompt.md
vlt plan --ai "add update user endpoint"
vlt plan --ai --provider openai "add update user endpoint"
vlt plan --ai --provider anthropic "add update user endpoint"
```

`vlt plan` must remain offline and deterministic by default. It should use project conventions, `.ai/project.md`, `.ai/symbols.json`, `.ai/routes.json`, `.ai/source-map.json`, route indexes, symbol indexes, file summaries, and deterministic templates.

Future `vlt plan --ai` may call an LLM provider, but it should send compact project context by default, not the whole codebase.

`vlt ai prompt` is the bridge before provider integration. It reads `.ai/project.md`, `.ai/symbols.json`, `.ai/routes.json`, `.ai/source-map.json`, `.ai/language-rules.md`, and the deterministic planner output, then renders a compact prompt for a developer to paste into Codex, Claude Code, or another coding agent. It never calls an external provider.

Supported prompt formats:

- `generic`: neutral Markdown prompt.
- `codex`: includes Codex-specific guidance to follow `AGENTS.md`.
- `claude`: includes Claude Code-specific guidance to follow `CLAUDE.md`.

## Native Route Context

Native route declarations are the primary source for `.ai/routes.json`:

```ts
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

The indexer records method, path, file, module, params, query, body type, success status/type, error status mappings, effects, and handler name. This lets AI agents answer “what endpoints exist?” from compact JSON without opening every route file, and it gives `vlt plan` and `vlt ai prompt` enough structure to suggest native syntax like `/users/{id}` instead of framework-specific `/users/:id`.

Legacy `// @route` comments and simple `app.get/post/put/patch/delete(...)` calls are still scanned as fallback metadata, but native route declarations have priority for the same method/path/file.

The route shape also drives real Rust/Axum code generation for API builds: params map to `axum::extract::Path`, query params map to `axum::extract::Query`, bodies map to `axum::Json<T>`, state maps to request context, success responses map to status plus JSON, typed `Err(error)` values map through route error status helpers, and all routes register on one `axum::Router`.

AI indexing and HTTP codegen are related but separate:

- `vlt ai index` records route metadata for planning, explanation, and prompt generation.
- `vlt build` lowers supported native route handlers and tiny inline route bodies into a generated Cargo/Axum project under `target/volt/rust-project`.
- Unsupported route body syntax fails with `EHTTPLOWER001` before Rust is written.

Agents should treat route declarations as HTTP contracts and put business logic in handler functions. Inline route bodies should stay inside the current lowering subset: simple `const`/`let` bindings, `return ok(...)`, literals, field access, array indexing, `array.length`, simple calls, and struct literals.

Prompted agents should also know the current core syntax: use owned values, use `let` for mutable locals, assign only to `let` bindings, keep `const` immutable, use field assignment only on mutable local structs, use `+=` and `-=` only on mutable numeric variables, use `++` and `--` only as standalone postfix statements on mutable numeric variables, use `array.push(value)` only on mutable arrays, use `array.length` for array length as `u64`, treat `array[index]` as returning an owned value that may clone and may panic if out of bounds, use `&&` and `||` only with bool operands, use `Array<T>` and `[a, b, c]` for arrays, annotate empty arrays such as `let ids: Array<u64> = []`, use `while (condition) { ... }` for condition-based loops, use `for item in array { ... }` over `Array<T>`, and use normal TypeScript-like `switch` for ordinary value branching. `for-in` currently consumes the array in generated Rust. `switch` is not pattern matching, has no fallthrough, and should not be used for `Option<T>`; use `if (value)` / `if (!value)` instead. Do not use `ctx.arena`; internal arenas are future implementation details only.

## Provider Interface

```rust
trait AiPlanner {
    fn plan(&self, input: AiPlanInput) -> Result<AiPlanOutput, AiPlanError>;
}
```

`AiPlanInput` should include:

- Task description
- `.ai/project.md`
- `.ai/symbols.json`
- `.ai/routes.json`
- `.ai/source-map.json`
- Relevant file summaries
- Language rules

`AiPlanOutput` should include:

- Inferred intent
- Files to edit
- Implementation steps
- Tests to add
- Risks
- Confidence

## Privacy And Scope

The default AI context should be compact and inspectable. Provider-backed planning should show or log the files and generated context being sent so developers can keep source exposure intentional.

## Current Limits

- No real LLM integration exists yet.
- `vlt plan --ai` is a design target, not implemented behavior.
- `vlt ai prompt` is implemented as deterministic prompt generation only.
- Route extraction uses native `route` declarations as primary metadata, with comments and simple `app.get/post/put/patch/delete(...)` calls as fallback.
- Native route codegen supports a minimal Axum vertical slice, not middleware, auth, database integration, or OpenAPI.
- Route handler lowering is v0.1-level and inline route body lowering is intentionally limited to the supported subset.
- No array `map`, `filter`, or `find` helpers yet.
- No safe `array.get`, nested field assignment, field assignment on Option-narrowed bindings, array element assignment, field compound assignment, prefix `++` / `--`, or `*=`, `/=`, `%=` yet.
- Call graph data is not complete yet.
