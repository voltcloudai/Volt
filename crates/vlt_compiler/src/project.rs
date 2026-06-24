use crate::{check_program, generate_rust, parse_source, DiagnosticBag, SourceFile};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("compiler diagnostics")]
    Diagnostics(DiagnosticBag),
    #[error("rustc failed:\n{0}")]
    Rustc(String),
    #[error("program failed:\n{0}")]
    Program(String),
}

pub type ProjectResult<T> = Result<T, ProjectError>;

#[derive(Debug, Clone)]
pub struct BuildOutput {
    pub rust_file: PathBuf,
    pub binary: PathBuf,
}

pub fn compile_file(path: &Path, output_root: &Path) -> ProjectResult<BuildOutput> {
    let source = SourceFile::from_path(path)?;
    let program = parse_source(&source).map_err(ProjectError::Diagnostics)?;
    check_program(&program, &source).map_err(ProjectError::Diagnostics)?;
    let rust = generate_rust(&program);

    let program_name = path
        .file_stem()
        .map(|stem| PathBuf::from(stem.to_os_string()))
        .unwrap_or_else(|| PathBuf::from("program"));
    let build_dir = output_root.join("build");
    std::fs::create_dir_all(&build_dir)?;

    let rust_file = build_dir.join(program_name.with_extension("rs"));
    std::fs::write(&rust_file, rust)?;

    let binary = output_root.join(program_name);
    let output = Command::new("rustc")
        .arg("--edition=2021")
        .arg(&rust_file)
        .arg("-o")
        .arg(&binary)
        .output()?;

    if !output.status.success() {
        return Err(ProjectError::Rustc(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(BuildOutput { rust_file, binary })
}

pub fn run_file(path: &Path, output_root: &Path) -> ProjectResult<String> {
    let build = compile_file(path, output_root)?;
    let output = Command::new(&build.binary).output()?;
    if !output.status.success() {
        return Err(ProjectError::Program(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
