use crate::{
    check_program, generate_axum_server, generate_rust, parse_source, DiagnosticBag, SourceFile,
};
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

pub fn compile_project(root: &Path, output_root: &Path) -> ProjectResult<BuildOutput> {
    let root = project_root(root);

    if !is_api_project(&root) {
        let entrypoint = root.join("src/main.vlt");
        return compile_file(&entrypoint, output_root);
    }

    let resolved = crate::resolver::resolve_project(&root).map_err(|err| match err {
        crate::resolver::ResolveError::Io(err) => ProjectError::Io(err),
        crate::resolver::ResolveError::Diagnostics(bag) => ProjectError::Diagnostics(bag),
    })?;

    let rust = generate_axum_server(&resolved.program, &resolved.source)
        .map_err(ProjectError::Diagnostics)?;

    let project_name = project_name(&root);
    let cargo_root = output_root.join("rust-project");
    let cargo_src = cargo_root.join("src");
    std::fs::create_dir_all(&cargo_src)?;
    std::fs::write(cargo_root.join("Cargo.toml"), api_cargo_toml(&project_name))?;

    let rust_file = cargo_src.join("main.rs");
    std::fs::write(&rust_file, rust)?;

    let output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .current_dir(&cargo_root)
        .output()?;

    if !output.status.success() {
        return Err(ProjectError::Rustc(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    std::fs::create_dir_all(output_root)?;

    let built_binary = cargo_root
        .join("target")
        .join("release")
        .join(binary_file_name(&project_name));

    let binary = output_root.join(binary_file_name(&project_name));
    std::fs::copy(&built_binary, &binary)?;

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

fn project_root(start: &Path) -> PathBuf {
    let mut current = if start.is_file() {
        start.parent().unwrap_or(start).to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        if current.join("volt.toml").exists() {
            return current;
        }
        if !current.pop() {
            return start.to_path_buf();
        }
    }
}

fn is_api_project(root: &Path) -> bool {
    std::fs::read_to_string(root.join("volt.toml"))
        .is_ok_and(|text| text.contains("type = \"api\""))
}

fn project_name(root: &Path) -> String {
    let text = std::fs::read_to_string(root.join("volt.toml")).unwrap_or_default();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name = ") {
            return rest.trim_matches('"').to_string();
        }
    }
    root.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "volt-api".to_string())
}

fn api_cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = {{ version = "0.8", features = ["json", "tokio", "http1"] }}
tokio = {{ version = "1", features = ["macros", "rt-multi-thread", "net"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
"#
    )
}

fn binary_file_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}
