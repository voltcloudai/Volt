# Claude Instructions

- Use `.ai/project.md`, `.ai/symbols.json`, `.ai/routes.json`, and `.ai/source-map.json` before reading broad source context.
- Prefer native `route method "path"` declarations for HTTP endpoints.
- Use `{id}` path params, not Express-style `:id` params.
- Use `Option<T>` and `none` for absence; do not use `null` or `undefined`.
- Use `if (value)` and `if (!value)` to narrow `Option<T>`.
- Use `error` declarations for domain errors and typed route errors like `errors UserError { UserNotFound 404 }`.
- Treat route declarations as HTTP contracts and put business logic in handler functions.
- Use handler argument order: params, query, body, ctx.
- Keep inline route bodies tiny and inside the supported Axum lowering subset unless you are extending the compiler.
- Run `vlt ai index` after source structure changes.
- Run `vlt build` after route changes.
- Do not add real LLM integration, middleware, auth, database integration, or OpenAPI generation in the current vertical slice.
