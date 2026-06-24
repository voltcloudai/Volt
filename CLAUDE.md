# Claude Instructions

- Use `.ai/project.md`, `.ai/symbols.json`, `.ai/routes.json`, and `.ai/source-map.json` before reading broad source context.
- Prefer native `route method "path"` declarations for HTTP endpoints.
- Use `{id}` path params, not Express-style `:id` params.
- Keep route bodies inside the supported Axum lowering subset unless you are extending the compiler.
- Run `vlt ai index` after source structure changes.
- Run `vlt build` after route changes.
- Do not add real LLM integration, middleware, auth, database integration, or OpenAPI generation in the current vertical slice.
