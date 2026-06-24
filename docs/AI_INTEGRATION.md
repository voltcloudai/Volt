# AI Integration Design

Volt's AI workflow is deterministic first.

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

The indexer records method, path, file, module, params, query, body type, success status/type, error status mappings, and effects. This lets AI agents answer “what endpoints exist?” from compact JSON without opening every route file, and it gives `vlt plan` and `vlt ai prompt` enough structure to suggest native syntax like `/users/{id}` instead of framework-specific `/users/:id`.

Legacy `// @route` comments and simple `app.get/post/put/patch/delete(...)` calls are still scanned as fallback metadata, but native route declarations have priority for the same method/path/file.

The route shape is also intentionally close to future Rust/Axum code generation: params map to `axum::extract::Path`, bodies map to `axum::Json<T>`, state maps to request context, success responses map to status plus JSON, and declared errors map to HTTP status responses. v0.1 generates visible route scaffolding and metadata; it does not implement a production HTTP runtime.

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
- Native route codegen is scaffolding only; full Axum handler lowering is not implemented yet.
- Call graph data is not complete yet.
