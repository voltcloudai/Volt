use crate::ast::{Decl, ErrorDecl, FunctionDecl, HttpMethod, RouteDecl, TypeDecl};
use crate::diagnostics::SourceFile;
use crate::parser::parse_source;
use crate::scaffold::{ARCHITECTURE, COMMANDS, EXAMPLES, LANGUAGE_RULES, MEMORY_MODEL};
use crate::types::Type;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiIndex {
    pub language: String,
    pub entrypoint: String,
    pub types: Vec<TypeInfo>,
    pub functions: Vec<FunctionInfo>,
    pub routes: Vec<RouteInfo>,
    pub errors: Vec<ErrorInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeInfo {
    pub name: String,
    pub file: String,
    pub module: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub file: String,
    pub module: String,
    pub signature: String,
    pub effects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    pub method: String,
    pub path: String,
    pub file: String,
    pub module: String,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
    #[serde(default)]
    pub query: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default)]
    pub success: RouteSuccessInfo,
    #[serde(default, rename = "errorType", skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<RouteErrorInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub handler: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub input: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSuccessInfo {
    pub status: u16,
    #[serde(rename = "type")]
    pub ty: String,
}

impl Default for RouteSuccessInfo {
    fn default() -> Self {
        Self {
            status: 200,
            ty: "Response".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteErrorInfo {
    pub name: String,
    pub status: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub name: String,
    pub file: String,
    pub module: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<ErrorVariantInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorVariantInfo {
    pub name: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMap {
    pub files: Vec<FileSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSummary {
    pub path: String,
    pub module: String,
    pub kind: String,
    pub types: Vec<TypeSummary>,
    pub functions: Vec<FunctionSummary>,
    pub routes: Vec<RouteInfo>,
    pub errors: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeSummary {
    pub name: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSummary {
    pub name: String,
    pub signature: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiPromptFormat {
    Generic,
    Codex,
    Claude,
}

#[derive(Debug, Clone)]
struct PlanContext {
    task: String,
    intent: String,
    confidence: String,
    reason: Option<String>,
    method: String,
    route_path: String,
    module: String,
    entrypoint: String,
    project_type: String,
    likely_files: Vec<String>,
    route_files: Vec<String>,
    service_files: Vec<String>,
    repository_files: Vec<String>,
    type_files: Vec<String>,
    error_files: Vec<String>,
    test_files: Vec<String>,
    types_to_update: Vec<String>,
    errors_to_consider: Vec<String>,
    existing_context: Vec<String>,
    steps: Vec<String>,
}

pub fn index_project(root: &Path) -> std::io::Result<AiIndex> {
    let root = project_root(root);
    let mut index = AiIndex {
        language: "Volt".to_string(),
        entrypoint: entrypoint(&root),
        types: Vec::new(),
        functions: Vec::new(),
        routes: Vec::new(),
        errors: Vec::new(),
    };
    let mut source_map = SourceMap { files: Vec::new() };

    for file in volt_files(&root)? {
        let rel = rel_path(&root, &file);
        let source_text = std::fs::read_to_string(&file)?;
        let module = file_module(&rel);
        let kind = file_kind(&rel);

        let (types, functions, declared_errors) = collect_symbols(&rel, &module, &source_text);
        let mut routes = collect_routes(&rel, &module, &source_text);
        dedupe_routes(&mut routes);

        index.types.extend(types.clone());
        index.functions.extend(functions.clone());
        index.routes.extend(routes.clone());
        index.errors.extend(declared_errors.clone());

        let errors = file_errors(&types, &routes, &declared_errors);
        source_map.files.push(FileSummary {
            path: rel.clone(),
            module,
            kind: kind.clone(),
            types: types
                .iter()
                .map(|ty| TypeSummary {
                    name: ty.name.clone(),
                    fields: ty.fields.clone(),
                })
                .collect(),
            functions: functions
                .iter()
                .map(|function| FunctionSummary {
                    name: function.name.clone(),
                    signature: function.signature.clone(),
                })
                .collect(),
            routes,
            errors,
            summary: file_summary(&rel, &kind),
        });
    }

    index.types.sort_by(|a, b| a.name.cmp(&b.name));
    index.functions.sort_by(|a, b| a.name.cmp(&b.name));
    index
        .routes
        .sort_by(|a, b| a.path.cmp(&b.path).then(a.method.cmp(&b.method)));
    dedupe_routes(&mut index.routes);
    merge_inferred_errors(&mut index);

    write_ai_files(&root, &index, &source_map)?;
    Ok(index)
}

pub fn load_index(root: &Path) -> std::io::Result<AiIndex> {
    let root = project_root(root);
    let text = std::fs::read_to_string(root.join(".ai/symbols.json"))?;
    serde_json::from_str(&text)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

pub fn load_source_map(root: &Path) -> std::io::Result<SourceMap> {
    let root = project_root(root);
    let text = std::fs::read_to_string(root.join(".ai/source-map.json"))?;
    serde_json::from_str(&text)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

pub fn ai_summary(root: &Path) -> std::io::Result<String> {
    let root = project_root(root);
    let project = std::fs::read_to_string(root.join(".ai/project.md")).unwrap_or_default();
    let index = load_index(&root)?;
    let source_map = load_source_map(&root).ok();
    let routes = load_routes(&root).unwrap_or_else(|_| index.routes.clone());

    let mut out = String::new();
    out.push_str("# Volt AI Context\n\n");
    out.push_str(project.trim());
    out.push_str("\n\n## Indexed Symbols\n\n");
    out.push_str(&format!("- Types: {}\n", index.types.len()));
    out.push_str(&format!("- Functions: {}\n", index.functions.len()));
    out.push_str(&format!("- Routes: {}\n", routes.len()));
    if let Some(source_map) = source_map {
        out.push_str(&format!("- Files: {}\n", source_map.files.len()));
    }

    if !routes.is_empty() {
        out.push_str("\n## Routes\n\n");
        for route in routes {
            let target = if route.handler.is_empty() {
                format!("{} {}", route.success.status, route.success.ty)
            } else {
                route.handler.clone()
            };
            out.push_str(&format!(
                "- {} {} -> `{}` ({})\n",
                route.method, route.path, target, route.file
            ));
        }
        out.push_str("\nNative routes lower to readable Rust/Axum handlers in API builds when their bodies stay inside the supported lowering subset.\n");
    }

    if !index.functions.is_empty() {
        out.push_str("\n## Functions\n\n");
        for function in &index.functions {
            out.push_str(&format!(
                "- `{}` in `{}`\n",
                function.signature, function.file
            ));
        }
    }

    Ok(out)
}

pub fn explain_symbol(root: &Path, symbol: &str) -> std::io::Result<Option<String>> {
    let index = load_index(root)?;
    if let Some(function) = index
        .functions
        .iter()
        .find(|function| function.name == symbol)
    {
        let related_routes = index
            .routes
            .iter()
            .filter(|route| route.handler == symbol || route.handler.contains(symbol))
            .map(|route| format!("{} {}", route.method, route.path))
            .collect::<Vec<_>>();
        let possible_errors = index
            .routes
            .iter()
            .filter(|route| route.handler == symbol || route.handler.contains(symbol))
            .flat_map(|route| route.errors.iter().map(|error| error.name.clone()))
            .collect::<BTreeSet<_>>();

        return Ok(Some(format!(
            "# Symbol: {symbol}\n\nFile: {}\nModule: {}\nSignature: {}\nEffects: {:?}\nCalled by: []\nCalls: []\nRelated routes: {:?}\nRelated types: {:?}\nPossible errors: {:?}\n",
            function.file,
            function.module,
            function.signature,
            function.effects,
            related_routes,
            related_types(&index, &function.signature),
            possible_errors.into_iter().collect::<Vec<_>>()
        )));
    }

    if let Some(ty) = index.types.iter().find(|ty| ty.name == symbol) {
        return Ok(Some(format!(
            "# Symbol: {symbol}\n\nFile: {}\nModule: {}\nSignature: type {} = {:?}\nEffects: []\nCalled by: []\nCalls: []\nRelated routes: {:?}\nRelated types: []\nPossible errors: []\n",
            ty.file,
            ty.module,
            ty.name,
            ty.fields,
            routes_for_type(&index, &ty.name)
        )));
    }

    if let Some(error) = index.errors.iter().find(|error| error.name == symbol) {
        let variants = error
            .variants
            .iter()
            .map(|variant| format!("{} {:?}", variant.name, variant.fields))
            .collect::<Vec<_>>();
        return Ok(Some(format!(
            "# Symbol: {symbol}\n\nFile: {}\nModule: {}\nSignature: error {}\nVariants: {:?}\nRelated routes: {:?}\n",
            error.file,
            error.module,
            error.name,
            variants,
            routes_for_type(&index, &error.name)
        )));
    }

    Ok(None)
}

pub fn plan_task(root: &Path, task: &str) -> std::io::Result<String> {
    let context = build_plan_context(root, task, true)?;
    Ok(render_plan(&context))
}

pub fn ai_prompt(root: &Path, task: &str, format: AiPromptFormat) -> std::io::Result<String> {
    let root = project_root(root);
    ensure_ai_metadata(&root)?;
    let _project = std::fs::read_to_string(root.join(".ai/project.md"))?;
    let _routes = std::fs::read_to_string(root.join(".ai/routes.json"))?;
    let _language_rules = std::fs::read_to_string(root.join(".ai/language-rules.md"))?;
    let context = build_plan_context(&root, task, false)?;
    Ok(render_prompt(&context, format))
}

fn build_plan_context(root: &Path, task: &str, allow_index: bool) -> std::io::Result<PlanContext> {
    let root = project_root(root);
    let index = if allow_index {
        load_index(&root).or_else(|_| index_project(&root))?
    } else {
        load_index(&root)?
    };
    let source_map = if allow_index {
        load_source_map(&root).unwrap_or_else(|_| SourceMap { files: Vec::new() })
    } else {
        load_source_map(&root)?
    };
    let lower = task.to_lowercase();
    let action = infer_action(&lower);
    let module = infer_module(&index, &source_map, &lower);
    let method = action_to_method(action);
    let module_name = module
        .as_deref()
        .unwrap_or_else(|| first_token_module(&lower).unwrap_or("unknown"));
    let existing_context = existing_context(&root, &source_map, module_name);
    let confidence = if module.is_some() && existing_context.has_core_files() {
        "high"
    } else if module.is_some() {
        "medium"
    } else {
        "low"
    };

    let route_path = suggested_route_path(&index, action, method, module_name);

    let files = likely_files(&root, module_name);
    let type_name = format!(
        "{}{}Input",
        pascal_action(action),
        singular_pascal(module_name)
    );
    let reason = if confidence == "low" {
        Some(format!(
            "No existing module matching \"{}\" was found.",
            module_name
        ))
    } else {
        None
    };

    Ok(PlanContext {
        task: task.trim().to_string(),
        intent: inferred_intent(action, module_name),
        confidence: confidence.to_string(),
        reason,
        method: method.to_string(),
        route_path,
        module: module_name.to_string(),
        entrypoint: index.entrypoint,
        project_type: project_type(&root).to_string(),
        likely_files: files,
        route_files: module_files(&source_map, module_name, "routes"),
        service_files: module_files(&source_map, module_name, "service"),
        repository_files: module_files(&source_map, module_name, "repository"),
        type_files: module_files(&source_map, module_name, "types"),
        error_files: module_files(&source_map, module_name, "errors"),
        test_files: module_files(&source_map, module_name, "test"),
        types_to_update: vec![type_name],
        errors_to_consider: errors_for(module_name, action)
            .into_iter()
            .map(ToString::to_string)
            .collect(),
        existing_context: existing_context.lines(module_name),
        steps: steps_for(module_name, action, method),
    })
}

fn render_plan(context: &PlanContext) -> String {
    let mut out = String::new();
    out.push_str("# Implementation plan\n\n");
    out.push_str(&format!("Task:\n{}\n\n", context.task));
    out.push_str(&format!("Inferred intent:\n{}\n\n", context.intent));
    out.push_str("Confidence:\n");
    out.push_str(&context.confidence);
    out.push_str("\n\n");
    if let Some(reason) = &context.reason {
        out.push_str("Reason:\n");
        out.push_str(reason);
        out.push_str("\n\n");
    }
    out.push_str("Suggested route:\n");
    out.push_str(&format!("{} {}\n\n", context.method, context.route_path));
    out.push_str("Likely files to edit:\n");
    for file in &context.likely_files {
        out.push_str(&format!("- {file}\n"));
    }
    out.push_str("\nTypes to add or update:\n");
    for ty in &context.types_to_update {
        out.push_str(&format!("- {ty}\n"));
    }
    out.push_str("\nErrors to consider:\n");
    for error in &context.errors_to_consider {
        out.push_str(&format!("- {error}\n"));
    }
    out.push_str("\nSuggested route error block:\n");
    out.push_str(&route_error_block(context));
    out.push('\n');
    out.push_str("\nRelevant existing context:\n");
    for item in &context.existing_context {
        out.push_str(&format!("- {item}\n"));
    }
    out.push_str("\nSuggested implementation steps:\n");
    for (idx, step) in context.steps.iter().enumerate() {
        out.push_str(&format!("{}. {step}\n", idx + 1));
    }
    out.push_str("\nAI context:\n");
    out.push_str("- Read `.ai/project.md`.\n");
    out.push_str("- Read `.ai/symbols.json`.\n");
    out.push_str("- Read `.ai/routes.json`.\n");
    out.push_str("- Read `.ai/source-map.json`.\n");
    out.push_str("- Then inspect only the likely files listed above.\n");
    out
}

fn render_prompt(context: &PlanContext, format: AiPromptFormat) -> String {
    let mut out = String::new();
    out.push_str("# AI implementation prompt\n\n");
    out.push_str("You are working in a Volt backend project.\n\n");
    out.push_str("Volt is a native backend language with TypeScript-like syntax. It is designed for high-performance server applications and AI-assisted software engineering.\n\n");
    out.push_str("## Task\n\n");
    out.push_str(&format!("{}.\n\n", capitalize_sentence(&context.task)));
    out.push_str("## Inferred intent\n\n");
    out.push_str(&context.intent);
    out.push_str("\n\n## Confidence\n\n");
    out.push_str(&context.confidence);
    out.push_str("\n\n");
    if let Some(reason) = &context.reason {
        out.push_str("## Reason\n\n");
        out.push_str(reason);
        out.push_str("\n\n## Recommended next step\n\n");
        out.push_str(
            "Create a new module or clarify which existing module should own this endpoint.\n\n",
        );
    }
    out.push_str("## Suggested route\n\n");
    out.push_str(&format!("{} {}\n\n", context.method, context.route_path));
    out.push_str("## Relevant project context\n\n");
    out.push_str(&format!(
        "- Project type: {}\n",
        context.project_type.to_uppercase()
    ));
    out.push_str(&format!("- Entrypoint: {}\n", context.entrypoint));
    out.push_str(&format!("- Module: {}\n", context.module));
    append_file_group(&mut out, "Existing route files", &context.route_files);
    append_file_group(&mut out, "Existing service files", &context.service_files);
    append_file_group(
        &mut out,
        "Existing repository files",
        &context.repository_files,
    );
    append_file_group(&mut out, "Existing type files", &context.type_files);
    append_file_group(&mut out, "Existing error files", &context.error_files);
    append_file_group(&mut out, "Existing test files", &context.test_files);
    out.push_str("\n## Likely files to edit\n\n");
    append_bullets(&mut out, &context.likely_files);
    out.push_str("\n## Types to add or update\n\n");
    append_bullets(&mut out, &context.types_to_update);
    out.push_str("\n## Errors to consider\n\n");
    append_bullets(&mut out, &context.errors_to_consider);
    out.push_str("\n## Suggested route error block\n\n");
    out.push_str("```ts\n");
    out.push_str(&route_error_block(context));
    out.push_str("```\n");
    out.push_str("\n## Expected behavior\n\n");
    out.push_str(&expected_behavior(&context.module));
    out.push_str("\n\n## Volt language rules\n\n");
    for rule in prompt_language_rules() {
        out.push_str(&format!("- {rule}\n"));
    }
    out.push_str("\n## Implementation steps\n\n");
    let mut steps = vec![
        "Read `.ai/project.md`.".to_string(),
        "Read `.ai/symbols.json`.".to_string(),
        "Read `.ai/routes.json`.".to_string(),
        "Read `.ai/source-map.json`.".to_string(),
        "Inspect the likely files listed above.".to_string(),
    ];
    steps.extend(context.steps.clone());
    for (idx, step) in steps.iter().enumerate() {
        out.push_str(&format!("{}. {step}\n", idx + 1));
    }
    out.push_str("\n## Validation commands\n\n");
    out.push_str("```bash\n");
    out.push_str("vlt ai index\n");
    out.push_str("vlt build\n");
    out.push_str("```\n\n");
    out.push_str("## Important constraints\n\n");
    out.push_str("- Do not call external AI APIs.\n");
    out.push_str("- Do not edit generated `.ai/*.json` files manually.\n");
    out.push_str("- Regenerate AI metadata using `vlt ai index`.\n");
    out.push_str("- Do not invent files outside the existing module structure unless necessary.\n");
    out.push_str("- If the task is ambiguous, explain the ambiguity before modifying code.\n");
    match format {
        AiPromptFormat::Generic => {}
        AiPromptFormat::Codex => {
            out.push_str("\n## Codex instructions\n\n");
            out.push_str("- Follow the repository `AGENTS.md`.\n");
            out.push_str("- Prefer small, testable changes.\n");
            out.push_str("- Run the validation commands before finishing.\n");
            out.push_str("- Keep native route bodies inside the supported Axum lowering subset unless you are extending the compiler.\n");
        }
        AiPromptFormat::Claude => {
            out.push_str("\n## Claude Code instructions\n\n");
            out.push_str("- Follow the repository `CLAUDE.md`.\n");
            out.push_str("- Keep the response concise.\n");
            out.push_str("- First inspect AI context files before opening source files.\n");
            out.push_str("- Prefer editing only the likely files listed in the prompt.\n");
        }
    }
    out
}

fn ensure_ai_metadata(root: &Path) -> std::io::Result<()> {
    for file in [
        ".ai/project.md",
        ".ai/symbols.json",
        ".ai/routes.json",
        ".ai/source-map.json",
        ".ai/language-rules.md",
    ] {
        if !root.join(file).exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("missing AI metadata file `{file}`; run `vlt ai index`"),
            ));
        }
    }
    Ok(())
}

fn module_files(source_map: &SourceMap, module: &str, kind: &str) -> Vec<String> {
    source_map
        .files
        .iter()
        .filter(|file| file.module == module && file.kind == kind)
        .map(|file| file.path.clone())
        .collect()
}

fn append_file_group(out: &mut String, label: &str, files: &[String]) {
    out.push_str(&format!("- {label}:\n"));
    if files.is_empty() {
        out.push_str("  - not found\n");
    } else {
        for file in files {
            out.push_str(&format!("  - {file}\n"));
        }
    }
}

fn append_bullets(out: &mut String, items: &[String]) {
    if items.is_empty() {
        out.push_str("- none\n");
    } else {
        for item in items {
            out.push_str(&format!("- {item}\n"));
        }
    }
}

fn expected_behavior(module: &str) -> String {
    if module == "users" {
        [
            "Implement an endpoint that updates an existing user.",
            "",
            "Suggested behavior:",
            "",
            "- Return success when the user exists and the input is valid.",
            "- Return validation error when the input is invalid.",
            "- Return not found when the user does not exist.",
            "- Return conflict when the new email is already used by another user.",
        ]
        .join("\n")
    } else {
        format!(
            "Implement the requested endpoint for the `{module}` module with explicit success and error behavior."
        )
    }
}

fn route_error_block(context: &PlanContext) -> String {
    let error_type = format!("{}Error", singular_pascal(&context.module));
    let mut out = format!("errors {error_type} {{\n");
    for error in &context.errors_to_consider {
        out.push_str(&format!(
            "  {error} {}\n",
            suggested_status_for_error(error)
        ));
    }
    out.push_str("}\n");
    out
}

fn suggested_status_for_error(error: &str) -> u16 {
    match error {
        "InvalidEmail" | "ValidationError" => 400,
        "UserNotFound" | "NotFound" => 404,
        "EmailAlreadyExists" | "Conflict" => 409,
        "DatabaseError" => 500,
        _ => 500,
    }
}

fn prompt_language_rules() -> Vec<&'static str> {
    vec![
        "Use TypeScript-like Volt syntax.",
        "Use native `route method \"path\"` declarations for HTTP endpoints.",
        "Prefer `/users/{id}` path params, not `/users/:id`.",
        "Prefer native routes over `app.get(...)` or `app.patch(...)` calls.",
        "Keep route bodies inside the Axum lowering subset: const bindings and `return ok(...)` over literals, field access, simple calls, and struct literals.",
        "Do not use null.",
        "Do not use undefined.",
        "Do not throw exceptions.",
        "Use Result<T, E> for fallible operations.",
        "Use Option<T> for absence and prefer `return none` for absent values.",
        "Prefer returning a plain value from Option<T> functions; the compiler wraps it.",
        "Use `if (value)` and `if (!value)` to narrow Option<T> values.",
        "Do not use truthiness for strings, numbers, or objects; use explicit comparisons.",
        "Use `error` declarations for domain errors instead of random strings.",
        "Prefer Result<T, DomainError> and call-style construction like `UserError.UserNotFound({ message: \"...\" })`.",
        "Prefer typed route errors: `errors UserError { UserNotFound 404 }`.",
        "Use explicit input/output route types.",
        "Use ctx.arena for request-scoped allocations when needed.",
        "Keep generated code simple and explicit.",
        "Do not introduce unsupported TypeScript syntax.",
    ]
}

fn capitalize_sentence(text: &str) -> String {
    let trimmed = text.trim();
    let mut chars = trimmed.chars();
    match chars.next() {
        Some(first) => format!(
            "{}{}",
            first.to_ascii_uppercase(),
            chars.collect::<String>()
        ),
        None => String::new(),
    }
}

fn write_ai_files(root: &Path, index: &AiIndex, source_map: &SourceMap) -> std::io::Result<()> {
    let ai_dir = root.join(".ai");
    let files_dir = ai_dir.join("files");
    std::fs::create_dir_all(ai_dir.join("tasks"))?;
    std::fs::create_dir_all(&files_dir)?;
    write_json(ai_dir.join("symbols.json"), index)?;
    write_json(ai_dir.join("routes.json"), &index.routes)?;
    write_json(ai_dir.join("errors.json"), &index.errors)?;
    write_json(ai_dir.join("source-map.json"), source_map)?;
    write_json(ai_dir.join("dependencies.json"), &serde_json::json!({}))?;
    std::fs::write(
        ai_dir.join("project.md"),
        project_md(root, index, source_map),
    )?;
    std::fs::write(ai_dir.join("commands.md"), COMMANDS)?;
    std::fs::write(ai_dir.join("architecture.md"), ARCHITECTURE)?;
    std::fs::write(ai_dir.join("language-rules.md"), LANGUAGE_RULES)?;
    std::fs::write(ai_dir.join("memory-model.md"), MEMORY_MODEL)?;
    std::fs::write(ai_dir.join("examples.md"), EXAMPLES)?;
    for file in &source_map.files {
        std::fs::write(
            files_dir.join(summary_file_name(&file.path)),
            render_file_summary(file),
        )?;
    }
    Ok(())
}

fn write_json(path: PathBuf, value: &impl Serialize) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    std::fs::write(path, format!("{json}\n"))
}

fn project_md(root: &Path, index: &AiIndex, source_map: &SourceMap) -> String {
    let modules = modules(index, source_map);
    format!(
        "# Volt Project Summary\n\nProject name: {}\nEntrypoint: {}\nProject type: {}\nModules:\n{}\nRoutes: {}\nTypes: {}\nFiles: {}\nCommands:\n- Check: `vlt check`\n- Test: `vlt test`\n- Format: `vlt fmt`\n- Build: `vlt build`\n- AI index: `vlt ai index`\n- Plan task: `vlt plan \"<task>\"`\n",
        project_name(root),
        index.entrypoint,
        project_type(root),
        modules
            .iter()
            .map(|module| format!("- {module}"))
            .collect::<Vec<_>>()
            .join("\n"),
        index.routes.len(),
        index.types.len(),
        source_map.files.len()
    )
}

fn collect_symbols(
    file: &str,
    module: &str,
    source_text: &str,
) -> (Vec<TypeInfo>, Vec<FunctionInfo>, Vec<ErrorInfo>) {
    let source = SourceFile::new(file, source_text.to_string());
    if let Ok(program) = parse_source(&source) {
        let mut types = Vec::new();
        let mut functions = Vec::new();
        let mut errors = Vec::new();
        for declaration in &program.declarations {
            match declaration {
                Decl::Import(_) => {}
                Decl::Type(type_decl) => types.push(type_info(file, module, type_decl)),
                Decl::Function(function) => functions.push(function_info(file, module, function)),
                Decl::Error(error_decl) => errors.push(error_info(file, module, error_decl)),
                Decl::Route(_) => {}
            }
        }
        (types, functions, errors)
    } else {
        (
            scan_types(file, module, source_text),
            scan_functions(file, module, source_text),
            Vec::new(),
        )
    }
}

fn type_info(file: &str, module: &str, type_decl: &TypeDecl) -> TypeInfo {
    let fields = type_decl
        .fields
        .iter()
        .map(|field| (field.name.clone(), type_to_string(&field.ty)))
        .collect();
    TypeInfo {
        name: type_decl.name.clone(),
        file: file.to_string(),
        module: module.to_string(),
        fields,
    }
}

fn function_info(file: &str, module: &str, function: &FunctionDecl) -> FunctionInfo {
    FunctionInfo {
        name: function.name.clone(),
        file: file.to_string(),
        module: module.to_string(),
        signature: function_signature(function),
        effects: Vec::new(),
    }
}

fn error_info(file: &str, module: &str, error_decl: &ErrorDecl) -> ErrorInfo {
    ErrorInfo {
        name: error_decl.name.clone(),
        file: file.to_string(),
        module: module.to_string(),
        variants: error_decl
            .variants
            .iter()
            .map(|variant| ErrorVariantInfo {
                name: variant.name.clone(),
                fields: variant
                    .fields
                    .iter()
                    .map(|field| (field.name.clone(), type_to_string(&field.ty)))
                    .collect(),
            })
            .collect(),
    }
}

fn function_signature(function: &FunctionDecl) -> String {
    let params = function
        .params
        .iter()
        .map(|param| format!("{}: {}", param.name, type_to_string(&param.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "function {}({}): {}",
        function.name,
        params,
        type_to_string(&function.return_type)
    )
}

fn scan_types(file: &str, module: &str, source: &str) -> Vec<TypeInfo> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut idx = 0;
    let mut types = Vec::new();
    while idx < lines.len() {
        let line = lines[idx].trim();
        if line.starts_with("type ") && line.contains("= {") {
            let name = line
                .trim_start_matches("type ")
                .split('=')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            let mut fields = BTreeMap::new();
            idx += 1;
            while idx < lines.len() && !lines[idx].trim().starts_with('}') {
                if let Some((field, ty)) = lines[idx].trim().split_once(':') {
                    fields.insert(
                        field.trim().to_string(),
                        ty.trim().trim_end_matches(',').to_string(),
                    );
                }
                idx += 1;
            }
            types.push(TypeInfo {
                name,
                file: file.to_string(),
                module: module.to_string(),
                fields,
            });
        }
        idx += 1;
    }
    types
}

fn scan_functions(file: &str, module: &str, source: &str) -> Vec<FunctionInfo> {
    let mut functions = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        if line.starts_with("function ") {
            let signature = line.split('{').next().unwrap_or(line).trim().to_string();
            let name = signature
                .trim_start_matches("function ")
                .split('(')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !name.is_empty() {
                functions.push(FunctionInfo {
                    name,
                    file: file.to_string(),
                    module: module.to_string(),
                    signature,
                    effects: Vec::new(),
                });
            }
        }
    }
    functions
}

fn collect_routes(file: &str, module: &str, source: &str) -> Vec<RouteInfo> {
    let mut routes = Vec::new();
    routes.extend(native_routes(file, module, source));
    let native_keys = routes.iter().map(route_key).collect::<BTreeSet<_>>();
    let mut fallback = Vec::new();
    fallback.extend(scan_route_metadata(file, module, source));
    fallback.extend(scan_legacy_route_comments(file, module, source));
    fallback.extend(scan_app_routes(file, module, source));
    fallback.extend(scan_route_blocks(file, module, source));
    routes.extend(
        fallback
            .into_iter()
            .filter(|route| !native_keys.contains(&route_key(route))),
    );
    routes
}

fn native_routes(file: &str, module: &str, source_text: &str) -> Vec<RouteInfo> {
    let source = SourceFile::new(file, source_text.to_string());
    let Ok(program) = parse_source(&source) else {
        return Vec::new();
    };
    program
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            Decl::Route(route) => Some(native_route_info(file, module, route)),
            _ => None,
        })
        .collect()
}

fn native_route_info(file: &str, module: &str, route: &RouteDecl) -> RouteInfo {
    RouteInfo {
        method: method_upper(route.method).to_string(),
        path: route.path.clone(),
        file: file.to_string(),
        module: module.to_string(),
        params: route
            .params
            .iter()
            .map(|field| (field.name.clone(), type_to_string(&field.ty)))
            .collect(),
        query: route
            .query
            .iter()
            .map(|field| (field.name.clone(), type_to_string(&field.ty)))
            .collect(),
        body: route.body_type.as_ref().map(type_to_string),
        success: RouteSuccessInfo {
            status: route.ok_status,
            ty: type_to_string(&route.ok_type),
        },
        error_type: route.error_type.as_ref().map(type_to_string),
        errors: route
            .errors
            .iter()
            .map(|error| RouteErrorInfo {
                name: error.name.clone(),
                status: error.status,
            })
            .collect(),
        effects: route.effects.clone(),
        handler: String::new(),
        input: String::new(),
        output: String::new(),
    }
}

struct LegacyRouteSpec {
    method: String,
    path: String,
    handler: String,
    input: String,
    output: String,
    errors: Vec<String>,
}

fn legacy_route(file: &str, module: &str, spec: LegacyRouteSpec) -> RouteInfo {
    let output = spec.output;
    RouteInfo {
        method: spec.method,
        path: spec.path,
        file: file.to_string(),
        module: module.to_string(),
        params: BTreeMap::new(),
        query: BTreeMap::new(),
        body: None,
        success: RouteSuccessInfo {
            status: 200,
            ty: if output.is_empty() {
                "Response".to_string()
            } else {
                output.clone()
            },
        },
        error_type: None,
        errors: spec
            .errors
            .into_iter()
            .map(|name| RouteErrorInfo { name, status: 500 })
            .collect(),
        effects: Vec::new(),
        handler: spec.handler,
        input: spec.input,
        output,
    }
}

fn method_upper(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn route_key(route: &RouteInfo) -> String {
    format!("{} {} {}", route.method, route.path, route.file)
}

fn scan_route_metadata(file: &str, module: &str, source: &str) -> Vec<RouteInfo> {
    let mut routes = Vec::new();
    let mut current: Option<RouteInfo> = None;

    for line in source.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("// @route ") {
            if let Some(route) = current.take() {
                routes.push(route);
            }
            let parts = rest.split_whitespace().collect::<Vec<_>>();
            if parts.len() >= 2 {
                current = Some(legacy_route(
                    file,
                    module,
                    LegacyRouteSpec {
                        method: parts[0].to_uppercase(),
                        path: parts[1].to_string(),
                        handler: String::new(),
                        input: String::new(),
                        output: String::new(),
                        errors: Vec::new(),
                    },
                ));
            }
        } else if let Some(route) = current.as_mut() {
            if let Some(handler) = line.strip_prefix("// @handler ") {
                route.handler = handler.trim().to_string();
            } else if let Some(input) = line.strip_prefix("// @input ") {
                route.input = input.trim().to_string();
            } else if let Some(output) = line.strip_prefix("// @output ") {
                route.output = output.trim().to_string();
                route.success.ty = route.output.clone();
            } else if let Some(errors) = line.strip_prefix("// @errors ") {
                route.errors = split_error_list(errors)
                    .into_iter()
                    .map(|name| RouteErrorInfo { name, status: 500 })
                    .collect();
            }
        }
    }

    if let Some(route) = current {
        routes.push(route);
    }

    routes
}

fn scan_legacy_route_comments(file: &str, module: &str, source: &str) -> Vec<RouteInfo> {
    let mut routes = Vec::new();
    for line in source.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("// route ") {
            let parts = rest.split_whitespace().collect::<Vec<_>>();
            if parts.len() >= 6 && parts[1].starts_with('/') {
                routes.push(legacy_route(
                    file,
                    module,
                    LegacyRouteSpec {
                        method: parts[0].to_uppercase(),
                        path: parts[1].to_string(),
                        handler: parts[2].to_string(),
                        input: parts[3].to_string(),
                        output: parts[4].to_string(),
                        errors: parts[5..].iter().map(|part| (*part).to_string()).collect(),
                    },
                ));
            }
        }
    }
    routes
}

fn scan_app_routes(file: &str, module: &str, source: &str) -> Vec<RouteInfo> {
    let mut routes = Vec::new();
    for line in source.lines().map(str::trim) {
        if line.starts_with("//") {
            continue;
        }
        for (needle, method) in [
            ("app.get(", "GET"),
            ("app.post(", "POST"),
            ("app.put(", "PUT"),
            ("app.patch(", "PATCH"),
            ("app.delete(", "DELETE"),
        ] {
            if let Some(start) = line.find(needle) {
                let rest = &line[start + needle.len()..];
                if let Some(path) = quoted_path(rest) {
                    let handler = route_call_handler(rest).unwrap_or_else(|| "inline".to_string());
                    routes.push(legacy_route(
                        file,
                        module,
                        LegacyRouteSpec {
                            method: method.to_string(),
                            path,
                            handler,
                            input: String::new(),
                            output: "Response".to_string(),
                            errors: Vec::new(),
                        },
                    ));
                }
            }
        }
    }
    routes
}

fn scan_route_blocks(file: &str, module: &str, source: &str) -> Vec<RouteInfo> {
    let mut routes = Vec::new();
    for line in source.lines().map(str::trim) {
        let Some(rest) = line.strip_prefix("route ") else {
            continue;
        };
        let mut pieces = rest.split_whitespace();
        let Some(method) = pieces.next() else {
            continue;
        };
        let Some(path) = quoted_path(rest) else {
            continue;
        };
        let (input, output, errors) = route_generic_types(rest);
        routes.push(legacy_route(
            file,
            module,
            LegacyRouteSpec {
                method: method.to_uppercase(),
                path,
                handler: route_handler_name(method, module),
                input,
                output,
                errors,
            },
        ));
    }
    routes
}

fn route_generic_types(text: &str) -> (String, String, Vec<String>) {
    let Some(start) = text.find("Route<") else {
        return (String::new(), String::new(), Vec::new());
    };
    let rest = &text[start + "Route<".len()..];
    let Some(end) = rest.find('>') else {
        return (String::new(), String::new(), Vec::new());
    };
    let parts = rest[..end]
        .split(',')
        .map(|part| part.trim().to_string())
        .collect::<Vec<_>>();
    let input = parts.first().cloned().unwrap_or_default();
    let output = parts.get(1).cloned().unwrap_or_default();
    let errors = parts.get(2).cloned().into_iter().collect();
    (input, output, errors)
}

fn route_handler_name(method: &str, module: &str) -> String {
    format!(
        "{}{}Handler",
        method.to_lowercase(),
        singular_pascal(module)
    )
}

fn quoted_path(text: &str) -> Option<String> {
    let start = text.find('"')?;
    let rest = &text[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn route_call_handler(text: &str) -> Option<String> {
    let first_quote = text.find('"')?;
    let rest = &text[first_quote + 1..];
    let second_quote = rest.find('"')?;
    let after_path = rest[second_quote + 1..].trim_start();
    let after_comma = after_path.strip_prefix(',')?.trim_start();
    if after_comma.starts_with('(') {
        return None;
    }
    let handler = after_comma
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    if handler.is_empty() {
        None
    } else {
        Some(handler)
    }
}

fn split_error_list(text: &str) -> Vec<String> {
    text.split([',', ' '])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn dedupe_routes(routes: &mut Vec<RouteInfo>) {
    let mut seen = BTreeSet::new();
    routes.retain(|route| seen.insert(route_key(route)));
}

fn merge_inferred_errors(index: &mut AiIndex) {
    let mut errors = BTreeMap::new();
    for error in &index.errors {
        errors.insert(error.name.clone(), error.clone());
    }
    for ty in &index.types {
        if ty.name.ends_with("Error") {
            errors.entry(ty.name.clone()).or_insert_with(|| ErrorInfo {
                name: ty.name.clone(),
                file: ty.file.clone(),
                module: ty.module.clone(),
                variants: Vec::new(),
            });
        }
    }
    for route in &index.routes {
        for error in &route.errors {
            errors.entry(error.name.clone()).or_insert(ErrorInfo {
                name: error.name.clone(),
                file: route.file.clone(),
                module: route.module.clone(),
                variants: Vec::new(),
            });
        }
    }
    index.errors = errors.into_values().collect();
}

fn file_errors(
    types: &[TypeInfo],
    routes: &[RouteInfo],
    declared_errors: &[ErrorInfo],
) -> Vec<String> {
    let mut errors = BTreeSet::new();
    for ty in types {
        if ty.name.ends_with("Error") {
            errors.insert(ty.name.clone());
        }
    }
    for error in declared_errors {
        errors.insert(error.name.clone());
    }
    for route in routes {
        if let Some(error_type) = &route.error_type {
            errors.insert(error_type.clone());
        }
        for error in &route.errors {
            errors.insert(error.name.clone());
        }
    }
    errors.into_iter().collect()
}

fn related_types(index: &AiIndex, signature: &str) -> Vec<String> {
    index
        .types
        .iter()
        .filter(|ty| signature.contains(&ty.name))
        .map(|ty| ty.name.clone())
        .collect()
}

fn routes_for_type(index: &AiIndex, ty: &str) -> Vec<String> {
    index
        .routes
        .iter()
        .filter(|route| {
            route.input == ty
                || route.output == ty
                || route.body.as_deref() == Some(ty)
                || route.success.ty == ty
                || route.error_type.as_deref() == Some(ty)
                || route.errors.iter().any(|err| err.name == ty)
        })
        .map(|route| format!("{} {}", route.method, route.path))
        .collect()
}

fn load_routes(root: &Path) -> std::io::Result<Vec<RouteInfo>> {
    let text = std::fs::read_to_string(project_root(root).join(".ai/routes.json"))?;
    serde_json::from_str(&text)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

fn project_root(start: &Path) -> PathBuf {
    let mut current = if start.is_file() {
        start.parent().unwrap_or(start).to_path_buf()
    } else {
        start.to_path_buf()
    };
    loop {
        if current.join("volt.toml").exists() || current.join("Cargo.toml").exists() {
            return current;
        }
        if !current.pop() {
            return start.to_path_buf();
        }
    }
}

fn volt_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_volt_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_volt_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .file_name()
            .is_some_and(|name| name == ".ai" || name == "target")
        {
            continue;
        }
        if path.is_dir() {
            collect_volt_files(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "vlt") {
            files.push(path);
        }
    }
    Ok(())
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn entrypoint(root: &Path) -> String {
    if root.join("src/main.vlt").exists() {
        "src/main.vlt".to_string()
    } else {
        "examples/hello.vlt".to_string()
    }
}

fn project_name(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "volt".to_string())
}

fn project_type(root: &Path) -> &'static str {
    if root.join("src").exists() {
        "api"
    } else {
        "compiler"
    }
}

fn file_module(path: &str) -> String {
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.first() == Some(&"src") {
        if parts.len() > 2 {
            parts[1].to_string()
        } else {
            "app".to_string()
        }
    } else if parts.first() == Some(&"examples") {
        "examples".to_string()
    } else {
        "root".to_string()
    }
}

fn file_kind(path: &str) -> String {
    for (suffix, kind) in [
        (".routes.vlt", "routes"),
        (".service.vlt", "service"),
        (".repository.vlt", "repository"),
        (".types.vlt", "types"),
        (".errors.vlt", "errors"),
        (".test.vlt", "test"),
    ] {
        if path.ends_with(suffix) {
            return kind.to_string();
        }
    }
    "source".to_string()
}

fn modules(index: &AiIndex, source_map: &SourceMap) -> Vec<String> {
    let mut modules = BTreeSet::new();
    for function in &index.functions {
        if !function.module.is_empty() && function.module != "app" && function.module != "root" {
            modules.insert(function.module.clone());
        }
    }
    for file in &source_map.files {
        if !file.module.is_empty() && file.module != "app" && file.module != "root" {
            modules.insert(file.module.clone());
        }
    }
    modules.into_iter().collect()
}

fn infer_module(index: &AiIndex, source_map: &SourceMap, task: &str) -> Option<String> {
    let module_names = modules(index, source_map);
    module_names
        .iter()
        .find(|module| task.contains(module.as_str()) || task.contains(singular(module)))
        .cloned()
        .or_else(|| {
            if task.contains("user") {
                Some("users".to_string())
            } else {
                None
            }
        })
}

fn first_token_module(task: &str) -> Option<&'static str> {
    if task.contains("billing") {
        Some("billing")
    } else if task.contains("payment") {
        Some("payments")
    } else if task.contains("order") {
        Some("orders")
    } else {
        None
    }
}

fn infer_action(task: &str) -> &'static str {
    if task.contains("update") || task.contains("edit") {
        "update"
    } else if task.contains("create") || task.contains("add") || task.contains("new") {
        "create"
    } else if task.contains("delete") || task.contains("remove") {
        "delete"
    } else if task.contains("get") || task.contains("find") || task.contains("fetch") {
        "get"
    } else {
        "change"
    }
}

fn action_to_method(action: &str) -> &'static str {
    match action {
        "update" => "PATCH",
        "create" => "POST",
        "delete" => "DELETE",
        "get" => "GET",
        _ => "POST",
    }
}

fn suggested_route_path(index: &AiIndex, action: &str, method: &str, module: &str) -> String {
    if matches!(action, "update" | "delete" | "get") {
        if let Some(route) = index.routes.iter().find(|route| {
            route.module == module && route.path.contains("{id}") && route.method == "GET"
        }) {
            return route.path.clone();
        }
    }
    if let Some(route) = index
        .routes
        .iter()
        .find(|route| route.module == module && route.method == method)
    {
        return route.path.clone();
    }
    match (action, module) {
        ("update", "users") | ("delete", "users") | ("get", "users") => "/users/{id}".to_string(),
        (_, "users") => "/users".to_string(),
        ("update", module) | ("delete", module) | ("get", module) => format!("/{module}/{{id}}"),
        (_, module) => format!("/{module}"),
    }
}

fn likely_files(root: &Path, module: &str) -> Vec<String> {
    let base = format!("src/{module}/{module}");
    let candidates = [
        format!("{base}.routes.vlt"),
        format!("{base}.service.vlt"),
        format!("{base}.repository.vlt"),
        format!("{base}.types.vlt"),
        format!("{base}.errors.vlt"),
        format!("{base}.test.vlt"),
    ];
    candidates
        .into_iter()
        .filter(|file| root.join(file).exists() || module == "users")
        .collect()
}

#[derive(Debug, Clone)]
struct ExistingContext {
    module_found: bool,
    routes: bool,
    service: bool,
    repository: bool,
    types: bool,
    errors: bool,
    tests: bool,
}

impl ExistingContext {
    fn has_core_files(&self) -> bool {
        self.module_found
            && self.routes
            && self.service
            && self.repository
            && self.types
            && self.errors
            && self.tests
    }

    fn lines(&self, module: &str) -> Vec<String> {
        let mut lines = Vec::new();
        if self.module_found {
            lines.push(format!("Existing {module} module found."));
        }
        if self.routes {
            lines.push("Existing route files found.".to_string());
        }
        if self.types && self.errors && self.service && self.repository {
            lines.push("Existing types/errors/service/repository files found.".to_string());
        }
        if self.tests {
            lines.push("Existing test files found.".to_string());
        }
        if lines.is_empty() {
            lines.push("No matching module context found.".to_string());
        }
        lines
    }
}

fn existing_context(root: &Path, source_map: &SourceMap, module: &str) -> ExistingContext {
    let module_found = source_map.files.iter().any(|file| file.module == module)
        || root.join(format!("src/{module}")).exists();
    ExistingContext {
        module_found,
        routes: root
            .join(format!("src/{module}/{module}.routes.vlt"))
            .exists(),
        service: root
            .join(format!("src/{module}/{module}.service.vlt"))
            .exists(),
        repository: root
            .join(format!("src/{module}/{module}.repository.vlt"))
            .exists(),
        types: root
            .join(format!("src/{module}/{module}.types.vlt"))
            .exists(),
        errors: root
            .join(format!("src/{module}/{module}.errors.vlt"))
            .exists(),
        tests: root
            .join(format!("src/{module}/{module}.test.vlt"))
            .exists(),
    }
}

fn pascal_action(action: &str) -> &'static str {
    match action {
        "update" => "Update",
        "create" => "Create",
        "delete" => "Delete",
        "get" => "Get",
        _ => "Change",
    }
}

fn singular_pascal(module: &str) -> String {
    let singular = singular(module);
    let mut chars = singular.chars();
    match chars.next() {
        Some(first) => format!(
            "{}{}",
            first.to_ascii_uppercase(),
            chars.collect::<String>()
        ),
        None => "Item".to_string(),
    }
}

fn singular(module: &str) -> &str {
    module.strip_suffix('s').unwrap_or(module)
}

fn errors_for(module: &str, action: &str) -> Vec<&'static str> {
    match (module, action) {
        ("users", "update") | ("users", "create") => vec![
            "UserNotFound",
            "InvalidEmail",
            "EmailAlreadyExists",
            "DatabaseError",
        ],
        ("users", "delete") | ("users", "get") => vec!["UserNotFound", "DatabaseError"],
        _ => vec!["ValidationError", "NotFound", "DatabaseError"],
    }
}

fn steps_for(module: &str, action: &str, method: &str) -> Vec<String> {
    let subject = singular_pascal(module);
    let input = format!("{}{}Input", pascal_action(action), subject);
    vec![
        format!("Add or update `{input}`."),
        format!("Add {action}-specific errors."),
        format!("Add `{action}{subject}` service function."),
        format!("Add repository {action} function."),
        format!("Add {method} route."),
        "Add tests for success, validation error, not found, and conflict.".to_string(),
        "Run `vlt ai index`.".to_string(),
        "Run `vlt fmt`.".to_string(),
        "Run `vlt check`.".to_string(),
        "Run `vlt build`.".to_string(),
        format!("Run `vlt test {module}`."),
    ]
}

fn inferred_intent(action: &str, module: &str) -> String {
    let singular = singular(module);
    match action {
        "update" => format!("Update an existing {singular}."),
        "create" => format!("Create a new {singular}."),
        "delete" => format!("Delete an existing {singular}."),
        "get" => format!("Fetch an existing {singular}."),
        _ => format!("Change the {singular} module."),
    }
}

fn type_to_string(ty: &Type) -> String {
    match ty {
        Type::Option(inner) => format!("Option<{}>", type_to_string(inner)),
        Type::Result(ok, err) => format!("Result<{}, {}>", type_to_string(ok), type_to_string(err)),
        other => other.to_string(),
    }
}

fn file_summary(path: &str, kind: &str) -> String {
    let module = file_module(path);
    match kind {
        "routes" => format!("Route handlers and route metadata for the {module} module."),
        "service" => format!("Business logic for the {module} module."),
        "repository" => format!("Storage access for the {module} module."),
        "types" => format!("Data types for the {module} module."),
        "errors" => format!("Error types for the {module} module."),
        "test" => format!("Tests for the {module} module."),
        _ => format!("Source file for the {module} module."),
    }
}

fn summary_file_name(path: &str) -> String {
    let path = path.trim_end_matches(".vlt");
    let mut out = String::new();
    for ch in path.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out.push_str(".summary.md");
    out
}

fn render_file_summary(file: &FileSummary) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", file.path));
    out.push_str(&format!("Module: {}\n", file.module));
    out.push_str(&format!("Kind: {}\n", file.kind));
    out.push_str(&format!("Summary: {}\n", file.summary));
    if !file.functions.is_empty() {
        out.push_str("\n## Functions\n\n");
        for function in &file.functions {
            out.push_str(&format!("- `{}`\n", function.signature));
        }
    }
    if !file.types.is_empty() {
        out.push_str("\n## Types\n\n");
        for ty in &file.types {
            out.push_str(&format!("- `{}`\n", ty.name));
        }
    }
    if !file.routes.is_empty() {
        out.push_str("\n## Routes\n\n");
        for route in &file.routes {
            out.push_str(&format!(
                "- {} {} -> {} {}\n",
                route.method, route.path, route.success.status, route.success.ty
            ));
        }
    }
    out
}
