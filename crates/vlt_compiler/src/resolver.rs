use crate::ast::*;
use crate::checker::{check_program_with_imports, ExternalSymbols, StructSig};
use crate::diagnostics::{Diagnostic, DiagnosticBag, SourceFile, Span};
use crate::parser::parse_source;
use crate::route_names::{route_params_type_name, route_query_type_name};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("compiler diagnostics")]
    Diagnostics(DiagnosticBag),
}

pub type ResolveResult<T> = Result<T, ResolveError>;

#[derive(Debug, Clone)]
pub struct ResolvedProject {
    pub root: PathBuf,
    pub program: Program,
    pub source: SourceFile,
}

#[derive(Debug, Clone)]
struct ModuleUnit {
    path: PathBuf,
    rel_path: String,
    source: SourceFile,
    program: Program,
}

#[derive(Debug, Clone, Default)]
struct ModuleExports {
    functions: HashMap<String, FunctionDecl>,
    types: HashMap<String, TypeDecl>,
    errors: HashMap<String, ErrorDecl>,
}

pub fn resolve_project(root: &Path) -> ResolveResult<ResolvedProject> {
    let root = project_root(root);
    let src = root.join("src");

    let mut files = Vec::new();
    collect_volt_files(&src, &mut files)?;
    files.sort();

    let mut diagnostics = DiagnosticBag::new();
    let mut modules = Vec::new();

    for path in files {
        let source = SourceFile::from_path(&path)?;
        match parse_source(&source) {
            Ok(program) => {
                let rel_path = rel_path(&root, &path);
                modules.push(ModuleUnit {
                    path,
                    rel_path,
                    source,
                    program,
                });
            }
            Err(bag) => append_diagnostics(&mut diagnostics, &bag),
        }
    }

    if !diagnostics.is_empty() {
        return Err(ResolveError::Diagnostics(diagnostics));
    }

    let exports = collect_exports(&modules, &mut diagnostics);
    let generated_route_types = collect_generated_route_types(&modules);

    for module in &modules {
        let external = resolve_imports(module, &exports, &generated_route_types, &mut diagnostics);
        if let Err(bag) =
            check_program_with_imports(&module.program, &module.source, external, false)
        {
            append_diagnostics(&mut diagnostics, &bag);
        }
    }

    let has_entrypoint = modules.iter().any(|module| {
        module.program.declarations.iter().any(|decl| match decl {
            Decl::Function(function) => function.name == "main",
            Decl::Route(_) => true,
            _ => false,
        })
    });

    if !has_entrypoint {
        let source = modules
            .first()
            .map(|module| &module.source)
            .cloned()
            .unwrap_or_else(|| SourceFile::new(root.join("src"), ""));
        diagnostics.push(Diagnostic::new(
            "E100",
            "missing project entrypoint",
            &source,
            Span::new(0, 0),
            Some("add `function main(): void { ... }` or at least one native `route`".to_string()),
        ));
    }

    if !diagnostics.is_empty() {
        return Err(ResolveError::Diagnostics(diagnostics));
    }

    let mut flattened = Vec::new();

    for module in &modules {
        for decl in &module.program.declarations {
            match decl {
                Decl::Import(_) => {}
                Decl::Function(_) | Decl::Type(_) | Decl::Error(_) | Decl::Route(_) => {
                    flattened.push(decl.clone())
                }
            }
        }
    }

    let synthetic_source = SourceFile::new(root.join("src"), synthetic_project_source(&modules));

    Ok(ResolvedProject {
        root,
        program: Program {
            declarations: flattened,
        },
        source: synthetic_source,
    })
}

fn collect_exports(
    modules: &[ModuleUnit],
    diagnostics: &mut DiagnosticBag,
) -> HashMap<PathBuf, ModuleExports> {
    let mut exports = HashMap::new();

    for module in modules {
        let mut module_exports = ModuleExports::default();

        for decl in &module.program.declarations {
            match decl {
                Decl::Function(function) if function.exported => {
                    if module_exports.functions.contains_key(&function.name) {
                        diagnostics.push(Diagnostic::new(
                            "ERESOLVE001",
                            format!("duplicate exported function `{}`", function.name),
                            &module.source,
                            function.span,
                            Some("exported symbols must be unique within a file".to_string()),
                        ));
                    }

                    module_exports
                        .functions
                        .insert(function.name.clone(), function.clone());
                }
                Decl::Type(type_decl) if type_decl.exported => {
                    if module_exports.types.contains_key(&type_decl.name)
                        || module_exports.errors.contains_key(&type_decl.name)
                    {
                        diagnostics.push(Diagnostic::new(
                            "ERESOLVE002",
                            format!("duplicate exported type `{}`", type_decl.name),
                            &module.source,
                            type_decl.span,
                            Some("exported symbols must be unique within a file".to_string()),
                        ));
                    }

                    module_exports
                        .types
                        .insert(type_decl.name.clone(), type_decl.clone());
                }
                Decl::Error(error_decl) if error_decl.exported => {
                    if module_exports.errors.contains_key(&error_decl.name)
                        || module_exports.types.contains_key(&error_decl.name)
                    {
                        diagnostics.push(Diagnostic::new(
                            "ERESOLVE006",
                            format!("duplicate exported error `{}`", error_decl.name),
                            &module.source,
                            error_decl.span,
                            Some("exported symbols must be unique within a file".to_string()),
                        ));
                    }

                    module_exports
                        .errors
                        .insert(error_decl.name.clone(), error_decl.clone());
                }
                Decl::Route(_) => {}
                Decl::Import(_) | Decl::Function(_) | Decl::Type(_) | Decl::Error(_) => {}
            }
        }

        exports.insert(normalize_path(&module.path), module_exports);
    }

    exports
}

fn resolve_imports(
    module: &ModuleUnit,
    exports: &HashMap<PathBuf, ModuleExports>,
    generated_route_types: &HashMap<String, StructSig>,
    diagnostics: &mut DiagnosticBag,
) -> ExternalSymbols {
    let mut external = ExternalSymbols::default();
    for (name, sig) in generated_route_types {
        external.insert_struct(name.clone(), sig.fields.clone());
    }
    external.insert_struct("Ctx", HashMap::new());

    for decl in &module.program.declarations {
        let Decl::Import(import) = decl else {
            continue;
        };

        let Some(target_path) = resolve_import_path(&module.path, &import.module) else {
            diagnostics.push(Diagnostic::new(
                "ERESOLVE003",
                format!("could not resolve import `{}`", import.module),
                &module.source,
                import.span,
                Some("use a relative import like `./users.types`".to_string()),
            ));
            continue;
        };

        let normalized = normalize_path(&target_path);
        let Some(target_exports) = exports.get(&normalized) else {
            diagnostics.push(Diagnostic::new(
                "ERESOLVE004",
                format!("import target `{}` was not found", import.module),
                &module.source,
                import.span,
                Some("check that the file exists and has `.vlt` extension".to_string()),
            ));
            continue;
        };

        for item in &import.items {
            if let Some(function) = target_exports.functions.get(&item.name) {
                external.insert_function(
                    function.name.clone(),
                    function
                        .params
                        .iter()
                        .map(|param| param.ty.clone())
                        .collect(),
                    function.return_type.clone(),
                );
                continue;
            }

            if let Some(type_decl) = target_exports.types.get(&item.name) {
                external.insert_struct(
                    type_decl.name.clone(),
                    type_decl
                        .fields
                        .iter()
                        .map(|field| (field.name.clone(), field.ty.clone()))
                        .collect(),
                );
                continue;
            }

            if let Some(error_decl) = target_exports.errors.get(&item.name) {
                external.insert_error(
                    error_decl.name.clone(),
                    error_decl
                        .variants
                        .iter()
                        .map(|variant| {
                            (
                                variant.name.clone(),
                                StructSig {
                                    fields: variant
                                        .fields
                                        .iter()
                                        .map(|field| (field.name.clone(), field.ty.clone()))
                                        .collect(),
                                },
                            )
                        })
                        .collect::<HashMap<String, StructSig>>(),
                );
                continue;
            }

            diagnostics.push(Diagnostic::new(
                "ERESOLVE005",
                format!("module `{}` does not export `{}`", import.module, item.name),
                &module.source,
                item.span,
                Some("export the symbol in the target module or fix the import name".to_string()),
            ));
        }
    }

    external
}

fn collect_generated_route_types(modules: &[ModuleUnit]) -> HashMap<String, StructSig> {
    let mut generated = HashMap::new();
    for module in modules {
        for decl in &module.program.declarations {
            let Decl::Route(route) = decl else {
                continue;
            };
            if !route.params.is_empty() {
                generated.insert(
                    route_params_type_name(route),
                    StructSig {
                        fields: route
                            .params
                            .iter()
                            .map(|field| (field.name.clone(), field.ty.clone()))
                            .collect(),
                    },
                );
            }
            if !route.query.is_empty() {
                generated.insert(
                    route_query_type_name(route),
                    StructSig {
                        fields: route
                            .query
                            .iter()
                            .map(|field| (field.name.clone(), field.ty.clone()))
                            .collect(),
                    },
                );
            }
        }
    }
    generated
}

fn resolve_import_path(from_file: &Path, module: &str) -> Option<PathBuf> {
    if !module.starts_with("./") && !module.starts_with("../") {
        return None;
    }

    let base = from_file.parent()?;
    let raw = base.join(module);

    let candidates = import_path_candidates(&raw, module);

    for candidate in &candidates {
        if candidate.exists() {
            return Some(candidate.clone());
        }
    }

    candidates.into_iter().next()
}

fn import_path_candidates(raw: &Path, module: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if module.ends_with(".vlt") {
        candidates.push(raw.to_path_buf());
    } else {
        candidates.push(append_vlt_extension(raw));
        candidates.push(raw.with_extension("vlt"));
        candidates.push(raw.to_path_buf());
    }

    candidates
}

fn append_vlt_extension(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(".vlt");
    PathBuf::from(value)
}

fn collect_volt_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_volt_files(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "vlt") {
            files.push(path);
        }
    }

    Ok(())
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

fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn synthetic_project_source(modules: &[ModuleUnit]) -> String {
    let mut out = String::new();

    for module in modules {
        out.push_str(&format!("// file: {}\n", module.rel_path));
        out.push_str(&module.source.source);
        out.push_str("\n\n");
    }

    out
}

fn append_diagnostics(target: &mut DiagnosticBag, source: &DiagnosticBag) {
    for diagnostic in source.all() {
        target.push(diagnostic.clone());
    }
}
