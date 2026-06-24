pub mod ai;
pub mod ast;
pub mod checker;
pub mod codegen_rust;
pub mod diagnostics;
pub mod formatter;
pub mod lexer;
pub mod parser;
pub mod project;
pub mod resolver;
pub mod scaffold;
pub mod types;

pub use checker::{check_program, check_program_with_imports, ExternalSymbols};
pub use codegen_rust::{generate_axum_server, generate_rust};
pub use diagnostics::{Diagnostic, DiagnosticBag, SourceFile, Span};
pub use formatter::format_program;
pub use parser::parse_source;
pub use resolver::{resolve_project, ResolvedProject};
