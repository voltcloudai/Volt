use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn merge(self, other: Span) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub source: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(path: impl Into<PathBuf>, source: impl Into<String>) -> Self {
        let source = source.into();
        let mut line_starts = vec![0];
        for (idx, ch) in source.char_indices() {
            if ch == '\n' {
                line_starts.push(idx + 1);
            }
        }

        Self {
            path: path.into(),
            source,
            line_starts,
        }
    }

    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        let source = std::fs::read_to_string(path)?;
        Ok(Self::new(path, source))
    }

    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        let col = offset.saturating_sub(self.line_starts[line_idx]) + 1;
        (line_idx + 1, col)
    }

    pub fn line_text(&self, line: usize) -> &str {
        let start = self.line_starts[line - 1];
        let end = self
            .line_starts
            .get(line)
            .copied()
            .unwrap_or(self.source.len());
        self.source[start..end].trim_end_matches(['\n', '\r'])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub span: Span,
    pub hint: Option<String>,
}

impl Diagnostic {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        source: &SourceFile,
        span: Span,
        hint: Option<String>,
    ) -> Self {
        let (line, column) = source.line_col(span.start);
        Self {
            code: code.into(),
            message: message.into(),
            file: source.path.clone(),
            line,
            column,
            span,
            hint,
        }
    }

    pub fn render(&self, source: &SourceFile) -> String {
        let line_text = source.line_text(self.line);
        let marker_len = self.span.end.saturating_sub(self.span.start).max(1);
        let marker = format!(
            "{}{}",
            " ".repeat(self.column.saturating_sub(1)),
            "^".repeat(marker_len.min(line_text.len().max(1)))
        );
        let hint = self
            .hint
            .as_ref()
            .map(|hint| format!(" {hint}"))
            .unwrap_or_default();

        format!(
            "error[{}]: {}\n  {}:{}:{}\n  |\n{} | {}\n  | {}{}\n",
            self.code,
            self.message,
            self.file.display(),
            self.line,
            self.column,
            self.line,
            line_text,
            marker,
            hint
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticBag {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    pub fn all(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn render(&self, source: &SourceFile) -> String {
        self.diagnostics
            .iter()
            .map(|diagnostic| diagnostic.render(source))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl fmt::Display for DiagnosticBag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for diagnostic in &self.diagnostics {
            writeln!(f, "error[{}]: {}", diagnostic.code, diagnostic.message)?;
        }
        Ok(())
    }
}
