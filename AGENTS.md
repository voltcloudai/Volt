# Agent Instructions

- This is a Rust workspace.
- Use `cargo test` after changes.
- Keep Volt syntax TypeScript-like, not Rust-like.
- Use native `route method "path"` syntax for HTTP endpoints.
- Use `{id}` path params, not Express-style `:id` params.
- Do not recommend `app.get(...)`/`app.patch(...)` as the primary route style.
- Do not add advanced language features unless tests are included.
- Prefer small compiler passes: parse, check, codegen.
- Keep generated Rust simple and readable.
- Keep v0.1 focused on the vertical slice: parse, check, generate Rust, build, run.
- Do not add async, HTTP, borrow checking, a package manager, or an LLVM backend until the core language is ready.
