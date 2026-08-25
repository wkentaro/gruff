use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use globset::Glob;
use globset::GlobMatcher;
use ignore::WalkBuilder;
use ruff_python_ast::Alias;
use ruff_python_ast::Comprehension;
use ruff_python_ast::Decorator;
use ruff_python_ast::ExceptHandler;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprContext;
use ruff_python_ast::ExprFString;
use ruff_python_ast::FStringPart;
use ruff_python_ast::Operator;
use ruff_python_ast::Parameter;
use ruff_python_ast::Pattern;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtFunctionDef;
use ruff_python_ast::TypeParams;
use ruff_python_ast::helpers::is_dunder;
use ruff_python_ast::token::TokenKind;
use ruff_python_ast::token::Tokens;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_decorator;
use ruff_python_ast::visitor::walk_except_handler;
use ruff_python_ast::visitor::walk_expr;
use ruff_python_ast::visitor::walk_pattern;
use ruff_python_ast::visitor::walk_stmt;
use ruff_python_parser::parse_module;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;
use serde::Deserialize;
use serde::Serialize;

const PRIVATE_CALL_WRAPPER_CODE: &str = "RH001";
const EXPLICIT_PRIVATE_INPUTS_CODE: &str = "RH002";
const RULE_CODES: &[&str] = &[PRIVATE_CALL_WRAPPER_CODE, EXPLICIT_PRIVATE_INPUTS_CODE];
const DEFAULT_EXCLUDES: &[&str] = &[
    ".bzr",
    ".direnv",
    ".eggs",
    ".git",
    ".git-rewrite",
    ".hg",
    ".ipynb_checkpoints",
    ".mypy_cache",
    ".nox",
    ".pants.d",
    ".pyenv",
    ".pytest_cache",
    ".pytype",
    ".ruff_cache",
    ".svn",
    ".tox",
    ".venv",
    ".vscode",
    "__pypackages__",
    "_build",
    "buck-out",
    "dist",
    "node_modules",
    "site-packages",
    "venv",
];

#[derive(Debug, Parser)]
#[command(name = "ruffhouse", version, about)]
pub struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run Ruffhouse on the given files or directories
    Check(CheckArguments),
}

#[derive(Debug, Args)]
struct CheckArguments {
    /// List of files or directories to check
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,

    /// Comma-separated list of rule codes to enable, or ALL to enable all rules
    #[arg(
        long,
        value_delimiter = ',',
        value_name = "RULE_CODE",
        help_heading = "Rule selection",
        next_line_help = true
    )]
    select: Option<Vec<String>>,

    /// Comma-separated list of rule codes to disable
    #[arg(
        long,
        value_delimiter = ',',
        value_name = "RULE_CODE",
        help_heading = "Rule selection",
        next_line_help = true
    )]
    ignore: Option<Vec<String>>,

    /// Output serialization format for violations
    #[arg(long, value_enum)]
    output_format: Option<OutputFormat>,

    /// Path to a pyproject.toml configuration file
    #[arg(
        long,
        value_name = "PYPROJECT_TOML",
        conflicts_with = "isolated",
        help_heading = "Global options"
    )]
    config: Option<PathBuf>,

    /// Ignore all configuration files
    #[arg(long, help_heading = "Global options")]
    isolated: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum OutputFormat {
    #[default]
    Full,
    Concise,
    Json,
    Github,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct RawConfig {
    lint: RawLintConfig,
    output_format: Option<OutputFormat>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct RawLintConfig {
    select: Vec<String>,
    ignore: Vec<String>,
    per_file_ignores: HashMap<String, Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct Pyproject {
    #[serde(default)]
    tool: ToolTable,
}

#[derive(Debug, Default, Deserialize)]
struct ToolTable {
    ruffhouse: Option<RawConfig>,
}

#[derive(Clone, Debug, Default)]
struct LoadedConfig {
    root: PathBuf,
    raw: RawConfig,
    per_file_ignores: Vec<PerFileIgnore>,
}

#[derive(Clone, Debug)]
struct PerFileIgnore {
    matcher: GlobMatcher,
    is_negated: bool,
    rules: Vec<String>,
}

#[derive(Debug)]
pub struct RunError(String);

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for RunError {}

#[derive(Clone, Debug, Serialize)]
struct Location {
    row: usize,
    column: usize,
}

#[derive(Clone, Debug)]
struct Finding {
    path: PathBuf,
    code: String,
    message: String,
    location: Location,
    end_location: Location,
    noqa_row: Option<usize>,
    source_line: String,
    caller: Option<Location>,
}

#[derive(Serialize)]
struct JsonFinding<'a> {
    cell: Option<usize>,
    code: &'a str,
    end_location: &'a Location,
    filename: String,
    fix: Option<()>,
    location: &'a Location,
    message: &'a str,
    name: &'a str,
    noqa_row: Option<usize>,
    severity: &'static str,
    url: Option<&'a str>,
}

pub fn run(arguments: Arguments) -> Result<u8, RunError> {
    match arguments.command {
        Command::Check(arguments) => run_check(arguments),
    }
}

fn run_check(arguments: CheckArguments) -> Result<u8, RunError> {
    if let Some(selected) = &arguments.select {
        validate_rules(selected)?;
    }
    if let Some(ignored) = &arguments.ignore {
        validate_rules(ignored)?;
    }
    let files = discover_files(&arguments.paths)?;
    let explicit_config = arguments.config.as_deref().map(load_config).transpose()?;
    let base_config = resolve_base_config(&arguments, explicit_config.as_ref())?;
    let output_format = arguments
        .output_format
        .or(base_config.raw.output_format)
        .unwrap_or_default();
    let mut config_cache = HashMap::new();
    let mut findings = Vec::new();
    let has_files = !files.is_empty();
    let mut is_rule_enabled_anywhere = false;

    for path in files {
        let config = resolve_config(
            &path,
            arguments.isolated,
            explicit_config.as_ref(),
            &mut config_cache,
        )?;
        let mut file_findings = check_file(&path)?;
        for code in RULE_CODES {
            let is_enabled = resolve_rule_enabled(&arguments, &config.raw, code)?;
            is_rule_enabled_anywhere |= is_enabled;
            if !is_enabled || is_ignored_for_file(&path, &config, code)? {
                file_findings.retain(|finding| finding.code != *code);
            }
        }
        findings.extend(file_findings);
    }

    if !has_files {
        for code in RULE_CODES {
            is_rule_enabled_anywhere |= resolve_rule_enabled(&arguments, &base_config.raw, code)?;
        }
    }

    if !is_rule_enabled_anywhere {
        eprintln!("warning: No rules are enabled; no policy analysis was performed");
    }

    findings.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.location.row.cmp(&right.location.row))
            .then(left.location.column.cmp(&right.location.column))
    });
    if print_findings(&findings, output_format)? {
        return Ok(0);
    }

    Ok(u8::from(!findings.is_empty()))
}

fn discover_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, RunError> {
    let mut files = BTreeMap::new();

    for path in paths {
        if path.is_file() {
            insert_file(&mut files, path, true)?;
            continue;
        }
        if !path.exists() {
            return Err(RunError(format!("Path does not exist: {}", path.display())));
        }
        if !path.is_dir() {
            return Err(RunError(format!(
                "Path is not a file or directory: {}",
                path.display()
            )));
        }

        let root = path.clone();
        for entry in WalkBuilder::new(path)
            .standard_filters(true)
            .hidden(false)
            .filter_entry(move |entry| {
                entry.path() == root
                    || !entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| DEFAULT_EXCLUDES.contains(&name))
            })
            .build()
        {
            let entry = entry.map_err(|error| RunError(error.to_string()))?;
            if entry.file_type().is_some_and(|kind| kind.is_file()) && is_python_file(entry.path())
            {
                insert_file(&mut files, entry.path(), false)?;
            }
        }
    }

    Ok(files.into_values().map(|(path, _)| path).collect())
}

fn insert_file(
    files: &mut BTreeMap<PathBuf, (PathBuf, bool)>,
    path: &Path,
    is_explicit: bool,
) -> Result<(), RunError> {
    let resolved = fs::canonicalize(path)
        .map_err(|error| RunError(format!("Failed to resolve {}: {error}", path.display())))?;
    let entry = files
        .entry(resolved)
        .or_insert_with(|| (path.to_path_buf(), is_explicit));
    if is_explicit && !entry.1 {
        *entry = (path.to_path_buf(), true);
    }
    Ok(())
}

fn is_python_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("py" | "pyi" | "pyw")
    )
}

fn resolve_base_config(
    arguments: &CheckArguments,
    explicit_config: Option<&LoadedConfig>,
) -> Result<LoadedConfig, RunError> {
    if arguments.isolated {
        return Ok(LoadedConfig::default());
    }
    if let Some(config) = explicit_config {
        return Ok(config.clone());
    }

    let first_path = arguments
        .paths
        .first()
        .expect("clap supplies the default input path");
    let search_directory = if first_path.is_dir() {
        first_path.as_path()
    } else {
        first_path.parent().unwrap_or_else(|| Path::new("."))
    };
    Ok(find_config(search_directory)?.unwrap_or_default())
}

fn resolve_config(
    path: &Path,
    isolated: bool,
    explicit_config: Option<&LoadedConfig>,
    cache: &mut HashMap<PathBuf, LoadedConfig>,
) -> Result<LoadedConfig, RunError> {
    if isolated {
        return Ok(LoadedConfig::default());
    }
    if let Some(config) = explicit_config {
        return Ok(config.clone());
    }

    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    if let Some(config) = cache.get(directory) {
        return Ok(config.clone());
    }

    let config = find_config(directory)?.unwrap_or_default();
    cache.insert(directory.to_path_buf(), config.clone());
    Ok(config)
}

fn find_config(start: &Path) -> Result<Option<LoadedConfig>, RunError> {
    let absolute_start = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| RunError(format!("Failed to read current directory: {error}")))?
            .join(start)
    };

    for directory in absolute_start.ancestors() {
        let path = directory.join("pyproject.toml");
        if !path.is_file() {
            continue;
        }
        let source = fs::read_to_string(&path)
            .map_err(|error| RunError(format!("Failed to read {}: {error}", path.display())))?;
        let document: Pyproject = toml::from_str(&source)
            .map_err(|error| RunError(format!("Failed to parse {}: {error}", path.display())))?;
        if let Some(raw) = document.tool.ruffhouse {
            return Ok(Some(load_raw_config(directory.to_path_buf(), raw)?));
        }
    }

    Ok(None)
}

fn load_config(path: &Path) -> Result<LoadedConfig, RunError> {
    let source = fs::read_to_string(path)
        .map_err(|error| RunError(format!("Failed to read {}: {error}", path.display())))?;
    let document: Pyproject = toml::from_str(&source)
        .map_err(|error| RunError(format!("Failed to parse {}: {error}", path.display())))?;
    let raw = document.tool.ruffhouse.ok_or_else(|| {
        RunError(format!(
            "{} does not contain [tool.ruffhouse]",
            path.display()
        ))
    })?;
    let root = std::env::current_dir()
        .map_err(|error| RunError(format!("Failed to read current directory: {error}")))?;
    load_raw_config(root, raw)
}

fn get_per_file_pattern(pattern: &str, root: &Path) -> String {
    let path = Path::new(pattern);
    if !path.is_absolute() {
        return pattern.to_owned();
    }
    if let Ok(relative) = path.strip_prefix(root) {
        return relative.display().to_string();
    }
    if let (Ok(path), Ok(root)) = (fs::canonicalize(path), fs::canonicalize(root))
        && let Ok(relative) = path.strip_prefix(root)
    {
        return relative.display().to_string();
    }
    pattern.to_owned()
}

fn load_raw_config(root: PathBuf, raw: RawConfig) -> Result<LoadedConfig, RunError> {
    validate_rules(&raw.lint.select)?;
    validate_rules(&raw.lint.ignore)?;
    let mut per_file_ignores = Vec::with_capacity(raw.lint.per_file_ignores.len());
    for (pattern, rules) in &raw.lint.per_file_ignores {
        validate_rules(rules)?;
        let (pattern, is_negated) = pattern
            .strip_prefix('!')
            .map_or((pattern.as_str(), false), |pattern| (pattern, true));
        let matcher_pattern = get_per_file_pattern(pattern, &root);
        let matcher = Glob::new(&matcher_pattern)
            .map_err(|error| RunError(format!("Invalid per-file ignore `{pattern}`: {error}")))?
            .compile_matcher();
        per_file_ignores.push(PerFileIgnore {
            matcher,
            is_negated,
            rules: rules.clone(),
        });
    }
    Ok(LoadedConfig {
        root,
        raw,
        per_file_ignores,
    })
}

fn resolve_rule_enabled(
    arguments: &CheckArguments,
    config: &RawConfig,
    code: &str,
) -> Result<bool, RunError> {
    let empty = Vec::new();
    if let Some(selected) = &arguments.select {
        let ignored = arguments.ignore.as_ref().unwrap_or(&empty);
        validate_rules(selected)?;
        validate_rules(ignored)?;
        return Ok(is_rule_enabled(selected, ignored, code));
    }

    let selected = &config.lint.select;
    let ignored = &config.lint.ignore;
    validate_rules(selected)?;
    validate_rules(ignored)?;
    if !is_rule_enabled(selected, ignored, code) {
        return Ok(false);
    }
    Ok(arguments
        .ignore
        .as_ref()
        .is_none_or(|ignored| get_rule_specificity(ignored, code).is_none()))
}

fn is_rule_enabled(selected: &[String], ignored: &[String], code: &str) -> bool {
    let selected = get_rule_specificity(selected, code);
    let ignored = get_rule_specificity(ignored, code);
    selected.is_some_and(|selected| ignored.is_none_or(|ignored| selected > ignored))
}

fn validate_rules(rules: &[String]) -> Result<(), RunError> {
    if let Some(rule) = rules.iter().find(|rule| !is_rule_selector(rule)) {
        return Err(RunError(format!("Unknown rule selector: {rule}")));
    }
    Ok(())
}

fn is_rule_selector(selector: &str) -> bool {
    selector == "ALL"
        || (!selector.is_empty() && RULE_CODES.iter().any(|code| code.starts_with(selector)))
}

fn get_rule_specificity(selectors: &[String], code: &str) -> Option<usize> {
    selectors
        .iter()
        .filter(|selector| **selector == "ALL" || code.starts_with(selector.as_str()))
        .map(|selector| if selector == "ALL" { 0 } else { selector.len() })
        .max()
}

fn is_ignored_for_file(path: &Path, config: &LoadedConfig, code: &str) -> Result<bool, RunError> {
    if config.per_file_ignores.is_empty() {
        return Ok(false);
    }
    let absolute_path = fs::canonicalize(path)
        .map_err(|error| RunError(format!("Failed to resolve {}: {error}", path.display())))?;
    let absolute_root = fs::canonicalize(&config.root).map_err(|error| {
        RunError(format!(
            "Failed to resolve configuration directory {}: {error}",
            config.root.display()
        ))
    })?;
    let relative_path = absolute_path
        .strip_prefix(&absolute_root)
        .unwrap_or(&absolute_path);

    for ignore in &config.per_file_ignores {
        if get_rule_specificity(&ignore.rules, code).is_none() {
            continue;
        }
        let is_match = ignore.matcher.is_match(&absolute_path)
            || ignore.matcher.is_match(relative_path)
            || relative_path
                .file_name()
                .is_some_and(|name| ignore.matcher.is_match(Path::new(name)));
        if is_match != ignore.is_negated {
            return Ok(true);
        }
    }

    Ok(false)
}

fn check_file(path: &Path) -> Result<Vec<Finding>, RunError> {
    let source = fs::read_to_string(path)
        .map_err(|error| RunError(format!("Failed to read {}: {error}", path.display())))?;
    Ok(check_source(path, &source))
}

fn check_source(path: &Path, source: &str) -> Vec<Finding> {
    let parsed = match parse_module(source) {
        Ok(parsed) => parsed,
        Err(error) => {
            let range = error.location;
            return vec![make_finding(
                path,
                "invalid-syntax",
                error.error.to_string(),
                range,
                source,
                None,
                None,
            )];
        }
    };
    let mut findings = Vec::new();

    for (definition, is_method) in find_private_definitions(parsed.suite()) {
        let name = definition.name.as_str();
        if !is_private_definition(name) || !has_implicit_private_inputs(definition, is_method) {
            continue;
        }

        let definition_range = definition.name.range;
        let noqa_row = find_noqa_row(source, parsed.tokens(), definition_range);
        if has_noqa(
            source,
            parsed.tokens(),
            noqa_row,
            EXPLICIT_PRIVATE_INPUTS_CODE,
        ) {
            continue;
        }

        findings.push(make_finding(
            path,
            EXPLICIT_PRIVATE_INPUTS_CODE,
            format!("Private definition `{name}` must receive required keyword-only inputs."),
            definition_range,
            source,
            None,
            Some(noqa_row),
        ));
    }

    for statement in parsed.suite() {
        let Stmt::FunctionDef(definition) = statement else {
            continue;
        };
        if !is_private_call_wrapper(definition) {
            continue;
        }

        let name = definition.name.as_str();
        if is_name_exported(parsed.suite(), name) {
            continue;
        }
        let mut references = ReferenceVisitor::new(
            name,
            definition.name.range,
            has_deferred_annotations(parsed.suite()),
        );
        for statement in parsed.suite() {
            references.visit_stmt(statement);
        }
        if references.loads.len() != 1
            || references.direct_calls.len() != 1
            || !references
                .direct_caller_calls
                .contains(&references.direct_calls[0])
            || references.definition_count != 1
            || references.has_other_reference
        {
            continue;
        }

        let definition_range = definition.name.range;
        let noqa_row = find_noqa_row(source, parsed.tokens(), definition_range);
        if has_noqa(source, parsed.tokens(), noqa_row, PRIVATE_CALL_WRAPPER_CODE) {
            continue;
        }

        findings.push(make_finding(
            path,
            PRIVATE_CALL_WRAPPER_CODE,
            format!("Private call wrapper `{name}` has one caller; inline it"),
            definition_range,
            source,
            Some(references.direct_calls[0]),
            Some(noqa_row),
        ));
    }

    findings
}

fn is_private_definition(name: &str) -> bool {
    name.starts_with('_') && !name.starts_with("__") && !name.ends_with('_')
}

fn has_implicit_private_inputs(definition: &StmtFunctionDef, is_method: bool) -> bool {
    let positional_count =
        definition.parameters.posonlyargs.len() + definition.parameters.args.len();
    let receiver_count =
        usize::from(is_method && !is_static_method(definition) && positional_count > 0);
    positional_count > receiver_count
        || definition
            .parameters
            .kwonlyargs
            .iter()
            .any(|parameter| parameter.default.is_some())
}

fn is_static_method(definition: &StmtFunctionDef) -> bool {
    definition.decorator_list.iter().any(
        |decorator| matches!(&decorator.expression, Expr::Name(name) if name.id == "staticmethod"),
    )
}

#[derive(Clone, Copy)]
enum DefinitionScope {
    Module,
    Class,
    Function,
}

fn find_private_definitions(statements: &[Stmt]) -> Vec<(&StmtFunctionDef, bool)> {
    let mut visitor = PrivateDefinitionVisitor {
        scope: DefinitionScope::Module,
        definitions: Vec::new(),
    };
    visitor.visit_body(statements);
    visitor.definitions
}

struct PrivateDefinitionVisitor<'a> {
    scope: DefinitionScope,
    definitions: Vec<(&'a StmtFunctionDef, bool)>,
}

impl<'a> Visitor<'a> for PrivateDefinitionVisitor<'a> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        let previous_scope = self.scope;
        match statement {
            Stmt::FunctionDef(definition) => {
                match previous_scope {
                    DefinitionScope::Module => self.definitions.push((definition, false)),
                    DefinitionScope::Class => self.definitions.push((definition, true)),
                    DefinitionScope::Function => {}
                }
                self.scope = DefinitionScope::Function;
            }
            Stmt::ClassDef(_) => self.scope = DefinitionScope::Class,
            _ => {}
        }
        walk_stmt(self, statement);
        self.scope = previous_scope;
    }
}

fn is_private_call_wrapper(definition: &StmtFunctionDef) -> bool {
    let name = definition.name.as_str();
    if !name.starts_with('_') || is_dunder(name) || !definition.decorator_list.is_empty() {
        return false;
    }

    let parameter_shadows_name = definition
        .parameters
        .iter()
        .any(|parameter| parameter.name().as_str() == name);
    match definition.body.as_slice() {
        [delegation] => extract_direct_call(delegation)
            .is_some_and(|call| !is_self_call(call, name) || parameter_shadows_name),
        [binding, delegation] => {
            let Some((binding_name, value)) = extract_binding(binding) else {
                return false;
            };
            if contains_forbidden_binding_expression(value) {
                return false;
            }
            let Some(call) = extract_direct_call(delegation) else {
                return false;
            };
            (!is_self_call(call, name) || binding_name == name || parameter_shadows_name)
                && contains_loaded_name(call, binding_name)
        }
        _ => false,
    }
}

fn is_self_call(expression: &Expr, function_name: &str) -> bool {
    let Expr::Call(call) = expression else {
        return false;
    };
    matches!(call.func.as_ref(), Expr::Name(name) if name.id == function_name)
}

fn extract_binding(statement: &Stmt) -> Option<(&str, &Expr)> {
    match statement {
        Stmt::Assign(assignment) if assignment.targets.len() == 1 => {
            let Expr::Name(target) = &assignment.targets[0] else {
                return None;
            };
            Some((target.id.as_str(), &assignment.value))
        }
        Stmt::AnnAssign(assignment) => {
            let Expr::Name(target) = assignment.target.as_ref() else {
                return None;
            };
            Some((target.id.as_str(), assignment.value.as_deref()?))
        }
        _ => None,
    }
}

fn extract_direct_call(statement: &Stmt) -> Option<&Expr> {
    let call = extract_root_call(statement)?;
    is_direct_delegated_call(call).then_some(call)
}

fn extract_root_call(statement: &Stmt) -> Option<&Expr> {
    let expression = match statement {
        Stmt::Return(statement) => statement.value.as_deref()?,
        Stmt::Expr(statement) => statement.value.as_ref(),
        _ => return None,
    };

    match expression {
        Expr::Call(_) => Some(expression),
        Expr::Await(await_expression)
            if matches!(await_expression.value.as_ref(), Expr::Call(_)) =>
        {
            Some(await_expression.value.as_ref())
        }
        _ => None,
    }
}

fn is_direct_delegated_call(expression: &Expr) -> bool {
    let mut visitor = DelegatedCallVisitor::default();
    visitor.visit_expr(expression);
    visitor.call_count == 1 && !visitor.has_control_expression
}

fn contains_forbidden_binding_expression(expression: &Expr) -> bool {
    let mut visitor = ForbiddenExpressionVisitor::default();
    visitor.visit_expr(expression);
    visitor.is_forbidden
}

fn contains_loaded_name(expression: &Expr, name: &str) -> bool {
    let mut visitor = LoadedNameVisitor {
        name,
        is_found: false,
    };
    visitor.visit_expr(expression);
    visitor.is_found
}

#[derive(Default)]
struct ForbiddenExpressionVisitor {
    is_forbidden: bool,
}

#[derive(Default)]
struct DelegatedCallVisitor {
    call_count: usize,
    has_control_expression: bool,
}

impl<'a> Visitor<'a> for DelegatedCallVisitor {
    fn visit_expr(&mut self, expression: &'a Expr) {
        match expression {
            Expr::Call(_) => self.call_count += 1,
            Expr::If(_)
            | Expr::Lambda(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_)
            | Expr::Generator(_)
            | Expr::Await(_)
            | Expr::Yield(_)
            | Expr::YieldFrom(_)
            | Expr::Named(_) => self.has_control_expression = true,
            _ => {}
        }
        walk_expr(self, expression);
    }
}

impl<'a> Visitor<'a> for ForbiddenExpressionVisitor {
    fn visit_expr(&mut self, expression: &'a Expr) {
        if matches!(
            expression,
            Expr::Call(_)
                | Expr::If(_)
                | Expr::BoolOp(_)
                | Expr::ListComp(_)
                | Expr::SetComp(_)
                | Expr::DictComp(_)
                | Expr::Generator(_)
                | Expr::Await(_)
                | Expr::Yield(_)
                | Expr::YieldFrom(_)
                | Expr::Lambda(_)
                | Expr::Named(_)
        ) {
            self.is_forbidden = true;
            return;
        }
        walk_expr(self, expression);
    }
}

struct LoadedNameVisitor<'a> {
    name: &'a str,
    is_found: bool,
}

impl<'a> Visitor<'a> for LoadedNameVisitor<'a> {
    fn visit_expr(&mut self, expression: &'a Expr) {
        if let Expr::Name(name) = expression
            && name.ctx == ExprContext::Load
            && name.id == self.name
        {
            self.is_found = true;
            return;
        }
        walk_expr(self, expression);
    }
}

struct ReferenceVisitor<'a> {
    name: &'a str,
    candidate_range: TextRange,
    loads: Vec<TextRange>,
    direct_calls: Vec<TextRange>,
    direct_caller_calls: Vec<TextRange>,
    definition_count: usize,
    has_other_reference: bool,
    is_suppressed: bool,
    lexical_is_suppressed: bool,
    is_class_body: bool,
    is_module_scope: bool,
    has_seen_candidate: bool,
    candidate_is_available: bool,
    annotations_are_deferred: bool,
}

impl<'a> ReferenceVisitor<'a> {
    fn new(name: &'a str, candidate_range: TextRange, annotations_are_deferred: bool) -> Self {
        Self {
            name,
            candidate_range,
            loads: Vec::new(),
            direct_calls: Vec::new(),
            direct_caller_calls: Vec::new(),
            definition_count: 0,
            has_other_reference: false,
            is_suppressed: false,
            lexical_is_suppressed: false,
            is_class_body: false,
            is_module_scope: true,
            has_seen_candidate: false,
            candidate_is_available: false,
            annotations_are_deferred,
        }
    }

    fn visit_function_definition(
        &mut self,
        definition: &'a StmtFunctionDef,
        evaluation_is_suppressed: bool,
        body_is_suppressed: bool,
    ) {
        let was_suppressed = self.is_suppressed;
        let was_lexical_is_suppressed = self.lexical_is_suppressed;
        let was_class_body = self.is_class_body;
        let was_module_scope = self.is_module_scope;
        let candidate_was_available = self.candidate_is_available;
        self.is_class_body = false;
        self.is_suppressed = evaluation_is_suppressed;
        for decorator in &definition.decorator_list {
            self.visit_decorator(decorator);
        }
        for parameter in definition.parameters.iter() {
            if let Some(default) = parameter.default() {
                self.visit_expr(default);
            }
        }

        let type_params_bind_name = definition
            .type_params
            .as_ref()
            .is_some_and(|params| do_type_params_bind_name(params, self.name));
        self.is_suppressed = evaluation_is_suppressed || type_params_bind_name;
        if let Some(type_params) = &definition.type_params {
            self.visit_type_params(type_params);
        }
        for parameter in definition.parameters.iter() {
            if let Some(annotation) = parameter.annotation() {
                self.visit_annotation(annotation);
            }
        }
        if let Some(returns) = &definition.returns {
            self.visit_annotation(returns);
        }

        self.is_suppressed = body_is_suppressed
            || type_params_bind_name
            || does_function_bind_name(definition, self.name);
        self.lexical_is_suppressed = self.is_suppressed;
        self.is_module_scope = false;
        self.candidate_is_available = true;
        self.visit_body(&definition.body);
        self.is_suppressed = was_suppressed;
        self.lexical_is_suppressed = was_lexical_is_suppressed;
        self.is_class_body = was_class_body;
        self.is_module_scope = was_module_scope;
        self.candidate_is_available = candidate_was_available;
    }

    fn visit_comprehension(
        &mut self,
        generators: &'a [Comprehension],
        elements: &[&'a Expr],
        is_lazy: bool,
    ) {
        let Some((first, rest)) = generators.split_first() else {
            for element in elements {
                self.visit_expr(element);
            }
            return;
        };
        let was_suppressed = self.is_suppressed;
        let was_lexical_is_suppressed = self.lexical_is_suppressed;
        let was_class_body = self.is_class_body;
        let was_module_scope = self.is_module_scope;
        let candidate_was_available = self.candidate_is_available;

        self.visit_expr(&first.iter);
        self.candidate_is_available |= is_lazy;
        self.is_class_body = false;
        self.is_module_scope = false;
        self.is_suppressed =
            self.lexical_is_suppressed || does_expression_bind_name(&first.target, self.name);
        self.lexical_is_suppressed = self.is_suppressed;
        self.visit_expr(&first.target);
        for condition in &first.ifs {
            self.visit_expr(condition);
        }
        for generator in rest {
            self.visit_expr(&generator.iter);
            self.is_suppressed |= does_expression_bind_name(&generator.target, self.name);
            self.lexical_is_suppressed = self.is_suppressed;
            self.visit_expr(&generator.target);
            for condition in &generator.ifs {
                self.visit_expr(condition);
            }
        }
        for element in elements {
            self.visit_expr(element);
        }

        self.is_suppressed = was_suppressed;
        self.lexical_is_suppressed = was_lexical_is_suppressed;
        self.is_class_body = was_class_body;
        self.is_module_scope = was_module_scope;
        self.candidate_is_available = candidate_was_available;
    }

    fn visit_class_statements(&mut self, statements: &'a [Stmt], class_scope_is_suppressed: bool) {
        let mut is_name_bound = false;
        for statement in statements {
            if let Stmt::FunctionDef(method) = statement {
                self.visit_function_definition(
                    method,
                    class_scope_is_suppressed || is_name_bound,
                    self.lexical_is_suppressed,
                );
                is_name_bound |= method.name.as_str() == self.name;
                continue;
            }
            self.is_class_body = true;
            self.is_suppressed = class_scope_is_suppressed || is_name_bound;
            self.visit_stmt(statement);
            if does_statement_delete_name(statement, self.name) {
                is_name_bound = false;
            } else {
                is_name_bound |= does_statement_bind_name(statement, self.name);
            }
        }
        self.is_suppressed = class_scope_is_suppressed;
    }
}

impl<'a> Visitor<'a> for ReferenceVisitor<'a> {
    fn visit_annotation(&mut self, annotation: &'a Expr) {
        let was_suppressed = self.is_suppressed;
        self.is_suppressed |= self.annotations_are_deferred;
        self.visit_expr(annotation);
        self.is_suppressed = was_suppressed;
    }

    fn visit_stmt(&mut self, statement: &'a Stmt) {
        if let Some(range) = find_direct_call_name_range(statement, self.name) {
            self.direct_caller_calls.push(range);
        }
        match statement {
            Stmt::FunctionDef(definition) => {
                let is_candidate_definition = definition.name.range == self.candidate_range;
                if is_candidate_definition
                    || (self.is_module_scope
                        && self.has_seen_candidate
                        && definition.name.as_str() == self.name)
                {
                    self.definition_count += 1;
                }
                if !is_candidate_definition
                    && does_function_rebind_global_name(definition, self.name)
                {
                    self.has_other_reference = true;
                }
                let is_suppressed = self.is_suppressed;
                let body_is_suppressed = if self.is_class_body {
                    self.lexical_is_suppressed
                } else {
                    is_suppressed
                };
                self.visit_function_definition(
                    definition,
                    is_suppressed || is_candidate_definition,
                    body_is_suppressed,
                );
                self.has_seen_candidate |= is_candidate_definition;
                self.candidate_is_available |= is_candidate_definition;
                return;
            }
            Stmt::ClassDef(definition) => {
                if does_body_rebind_global_name(&definition.body, self.name) {
                    self.has_other_reference = true;
                }
                if self.is_module_scope
                    && self.has_seen_candidate
                    && definition.name.as_str() == self.name
                {
                    self.definition_count += 1;
                }
                for decorator in &definition.decorator_list {
                    self.visit_decorator(decorator);
                }
                let was_suppressed = self.is_suppressed;
                let was_class_body = self.is_class_body;
                let was_module_scope = self.is_module_scope;
                let type_params_bind_name = definition
                    .type_params
                    .as_ref()
                    .is_some_and(|params| do_type_params_bind_name(params, self.name));
                self.is_suppressed = was_suppressed || type_params_bind_name;
                self.is_module_scope = false;
                if let Some(type_params) = &definition.type_params {
                    self.visit_type_params(type_params);
                }
                if let Some(arguments) = &definition.arguments {
                    self.visit_arguments(arguments);
                }
                let class_scope_is_suppressed = self.lexical_is_suppressed
                    || (was_module_scope && !self.has_seen_candidate)
                    || type_params_bind_name;
                self.is_suppressed = class_scope_is_suppressed;
                self.visit_class_statements(&definition.body, class_scope_is_suppressed);
                self.is_suppressed = was_suppressed;
                self.is_class_body = was_class_body;
                self.is_module_scope = was_module_scope;
                return;
            }
            Stmt::TypeAlias(alias) => {
                let was_suppressed = self.is_suppressed;
                let candidate_was_available = self.candidate_is_available;
                self.is_suppressed |= alias
                    .type_params
                    .as_ref()
                    .is_some_and(|params| do_type_params_bind_name(params, self.name));
                if let Some(type_params) = &alias.type_params {
                    self.visit_type_params(type_params);
                }
                self.candidate_is_available = true;
                self.visit_expr(&alias.value);
                self.candidate_is_available = candidate_was_available;
                self.is_suppressed = was_suppressed;
                self.visit_expr(&alias.name);
                return;
            }
            Stmt::AugAssign(assignment)
                if self.is_class_body
                    && !self.is_suppressed
                    && does_expression_bind_name(&assignment.target, self.name) =>
            {
                self.has_other_reference = true;
            }
            Stmt::For(for_statement) if self.is_class_body => {
                self.visit_expr(&for_statement.iter);
                let was_suppressed = self.is_suppressed;
                self.is_suppressed |= does_expression_bind_name(&for_statement.iter, self.name)
                    || does_expression_bind_name(&for_statement.target, self.name);
                self.visit_expr(&for_statement.target);
                let body_is_suppressed = self.is_suppressed;
                self.visit_class_statements(&for_statement.body, body_is_suppressed);
                let body_binds_name = for_statement
                    .body
                    .iter()
                    .any(|statement| does_statement_bind_name(statement, self.name));
                self.visit_class_statements(
                    &for_statement.orelse,
                    body_is_suppressed || body_binds_name,
                );
                self.is_suppressed = was_suppressed;
                return;
            }
            Stmt::With(with_statement) if self.is_class_body => {
                let was_suppressed = self.is_suppressed;
                for item in &with_statement.items {
                    self.visit_expr(&item.context_expr);
                    self.is_suppressed |= does_expression_bind_name(&item.context_expr, self.name);
                    if let Some(target) = &item.optional_vars {
                        self.is_suppressed |= does_expression_bind_name(target, self.name);
                        self.visit_expr(target);
                    }
                }
                self.visit_class_statements(&with_statement.body, self.is_suppressed);
                self.is_suppressed = was_suppressed;
                return;
            }
            Stmt::Match(match_statement) if self.is_class_body => {
                self.visit_expr(&match_statement.subject);
                let was_suppressed = self.is_suppressed
                    || does_expression_bind_name(&match_statement.subject, self.name);
                for case in &match_statement.cases {
                    self.is_suppressed = was_suppressed
                        || does_pattern_recursively_bind_name(&case.pattern, self.name);
                    self.visit_pattern(&case.pattern);
                    if let Some(guard) = &case.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_class_statements(&case.body, self.is_suppressed);
                }
                self.is_suppressed = was_suppressed;
                return;
            }
            Stmt::If(if_statement) if self.is_class_body => {
                let was_suppressed = self.is_suppressed;
                self.visit_expr(&if_statement.test);
                let mut class_scope_is_suppressed =
                    was_suppressed || does_expression_bind_name(&if_statement.test, self.name);
                self.visit_class_statements(&if_statement.body, class_scope_is_suppressed);
                for clause in &if_statement.elif_else_clauses {
                    self.is_suppressed = class_scope_is_suppressed;
                    if let Some(test) = &clause.test {
                        self.visit_expr(test);
                        class_scope_is_suppressed |= does_expression_bind_name(test, self.name);
                    }
                    self.visit_class_statements(&clause.body, class_scope_is_suppressed);
                }
                self.is_suppressed = class_scope_is_suppressed;
                return;
            }
            Stmt::Try(try_statement) if self.is_class_body => {
                let was_suppressed = self.is_suppressed;
                self.visit_class_statements(&try_statement.body, was_suppressed);
                let body_binds_name = try_statement
                    .body
                    .iter()
                    .any(|statement| does_statement_bind_name(statement, self.name));
                for handler in &try_statement.handlers {
                    self.is_suppressed = was_suppressed || body_binds_name;
                    self.visit_except_handler(handler);
                }
                self.visit_class_statements(
                    &try_statement.orelse,
                    was_suppressed || body_binds_name,
                );
                let try_binds_name = body_binds_name
                    || try_statement
                        .handlers
                        .iter()
                        .any(|handler| does_handler_body_bind_name(handler, self.name))
                    || try_statement
                        .orelse
                        .iter()
                        .any(|statement| does_statement_bind_name(statement, self.name));
                self.visit_class_statements(
                    &try_statement.finalbody,
                    was_suppressed || try_binds_name,
                );
                self.is_suppressed = was_suppressed;
                return;
            }
            Stmt::While(while_statement) if self.is_class_body => {
                let was_suppressed = self.is_suppressed;
                self.visit_expr(&while_statement.test);
                let body_is_suppressed =
                    was_suppressed || does_expression_bind_name(&while_statement.test, self.name);
                self.visit_class_statements(&while_statement.body, body_is_suppressed);
                self.visit_class_statements(&while_statement.orelse, body_is_suppressed);
                self.is_suppressed = was_suppressed;
                return;
            }
            _ => {}
        }
        walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'a Expr) {
        match expression {
            Expr::ListComp(comprehension) => {
                self.visit_comprehension(&comprehension.generators, &[&comprehension.elt], false);
                return;
            }
            Expr::SetComp(comprehension) => {
                self.visit_comprehension(&comprehension.generators, &[&comprehension.elt], false);
                return;
            }
            Expr::Generator(comprehension) => {
                self.visit_comprehension(&comprehension.generators, &[&comprehension.elt], true);
                return;
            }
            Expr::DictComp(comprehension) => {
                if let Some(key) = &comprehension.key {
                    self.visit_comprehension(
                        &comprehension.generators,
                        &[key, &comprehension.value],
                        false,
                    );
                } else {
                    self.visit_comprehension(
                        &comprehension.generators,
                        &[&comprehension.value],
                        false,
                    );
                }
                return;
            }
            _ => {}
        }
        if let Expr::Lambda(lambda) = expression {
            let was_suppressed = self.is_suppressed;
            let was_lexical_is_suppressed = self.lexical_is_suppressed;
            let was_class_body = self.is_class_body;
            let was_module_scope = self.is_module_scope;
            if let Some(parameters) = &lambda.parameters {
                for parameter in parameters.iter() {
                    if let Some(default) = parameter.default() {
                        self.visit_expr(default);
                    }
                }
                self.is_suppressed = self.lexical_is_suppressed
                    || parameters
                        .iter()
                        .any(|parameter| parameter.name().as_str() == self.name)
                    || does_expression_bind_name(&lambda.body, self.name);
            } else {
                self.is_suppressed = self.lexical_is_suppressed
                    || does_expression_bind_name(&lambda.body, self.name);
            }
            self.is_class_body = false;
            self.is_module_scope = false;
            self.lexical_is_suppressed = self.is_suppressed;
            let candidate_was_available = self.candidate_is_available;
            self.candidate_is_available = true;
            self.visit_expr(&lambda.body);
            self.is_suppressed = was_suppressed;
            self.lexical_is_suppressed = was_lexical_is_suppressed;
            self.is_class_body = was_class_body;
            self.is_module_scope = was_module_scope;
            self.candidate_is_available = candidate_was_available;
            return;
        }
        if self.is_suppressed {
            walk_expr(self, expression);
            return;
        }
        match expression {
            Expr::Call(call) => {
                if let Expr::Name(name) = call.func.as_ref()
                    && name.ctx == ExprContext::Load
                    && name.id == self.name
                    && self.candidate_is_available
                {
                    self.direct_calls.push(name.range);
                }
            }
            Expr::Name(name) if name.id == self.name => {
                if !self.candidate_is_available {
                    walk_expr(self, expression);
                    return;
                }
                if name.ctx == ExprContext::Load {
                    self.loads.push(name.range);
                } else if !self.is_class_body {
                    self.has_other_reference = true;
                }
            }
            _ => {}
        }
        walk_expr(self, expression);
    }

    fn visit_parameter(&mut self, parameter: &'a Parameter) {
        if parameter.name.as_str() == self.name {
            self.has_other_reference = true;
        }
        if let Some(annotation) = parameter.annotation() {
            self.visit_annotation(annotation);
        }
    }

    fn visit_alias(&mut self, alias: &'a Alias) {
        if alias.name.as_str() == "*" && self.is_module_scope && self.has_seen_candidate {
            self.has_other_reference = true;
            return;
        }
        let bound_name = get_bound_alias_name(alias);
        if !self.is_class_body
            && !self.is_suppressed
            && (!self.is_module_scope || self.has_seen_candidate)
            && bound_name == self.name
        {
            self.has_other_reference = true;
        }
    }

    fn visit_except_handler(&mut self, handler: &'a ExceptHandler) {
        if self.is_class_body {
            let ExceptHandler::ExceptHandler(handler) = handler;
            if let Some(type_) = &handler.type_ {
                self.visit_expr(type_);
            }
            let was_suppressed = self.is_suppressed;
            self.is_suppressed |= handler.name.as_ref().is_some_and(|name| name == self.name);
            self.visit_class_statements(&handler.body, self.is_suppressed);
            self.is_suppressed = was_suppressed;
            return;
        }
        if !self.is_suppressed
            && !self.is_class_body
            && does_exception_bind_name(handler, self.name)
        {
            self.has_other_reference = true;
        }
        walk_except_handler(self, handler);
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        if !self.is_suppressed && !self.is_class_body && does_pattern_bind_name(pattern, self.name)
        {
            self.has_other_reference = true;
        }
        walk_pattern(self, pattern);
    }

    fn visit_decorator(&mut self, decorator: &'a Decorator) {
        let load_count = self.loads.len();
        walk_decorator(self, decorator);
        if self.loads.len() > load_count {
            self.has_other_reference = true;
        }
    }
}

fn do_type_params_bind_name(type_params: &TypeParams, name: &str) -> bool {
    type_params
        .iter()
        .any(|parameter| parameter.name().as_str() == name)
}

fn find_direct_call_name_range(statement: &Stmt, name: &str) -> Option<TextRange> {
    let Expr::Call(call) = extract_root_call(statement)? else {
        return None;
    };
    let Expr::Name(target) = call.func.as_ref() else {
        return None;
    };
    (target.ctx == ExprContext::Load && target.id == name).then_some(target.range)
}

fn get_bound_alias_name(alias: &Alias) -> &str {
    alias.asname.as_ref().map_or_else(
        || alias.name.as_str().split('.').next().unwrap_or_default(),
        |name| name.as_str(),
    )
}

fn get_static_string(expression: &Expr) -> Option<String> {
    match expression {
        Expr::StringLiteral(string) => Some(string.value.to_str().to_owned()),
        Expr::FString(string) => get_static_fstring(string),
        Expr::BinOp(binary) if binary.op == Operator::Add => {
            let mut value = get_static_string(&binary.left)?;
            value.push_str(&get_static_string(&binary.right)?);
            Some(value)
        }
        _ => None,
    }
}

fn get_static_fstring(string: &ExprFString) -> Option<String> {
    let mut value = String::new();
    for part in &string.value {
        match part {
            FStringPart::Literal(literal) => value.push_str(literal.as_str()),
            FStringPart::FString(string) => {
                if string.elements.interpolations().next().is_some() {
                    return None;
                }
                for literal in string.elements.literals() {
                    value.push_str(literal);
                }
            }
        }
    }
    Some(value)
}

fn is_name_exported(statements: &[Stmt], name: &str) -> bool {
    let mut aliases = HashMap::new();
    let mut all_aliases = HashSet::new();
    let mut export_count = 0;
    for statement in statements {
        match statement {
            Stmt::Assign(assignment) if assignment.targets.len() == 1 => {
                if let Expr::Name(target) = &assignment.targets[0] {
                    if target.id != "__all__" {
                        all_aliases.remove(target.id.as_str());
                    }
                    let count = count_export_names(&assignment.value, name, &aliases);
                    if count > 0 {
                        aliases.insert(target.id.to_string(), count);
                    } else {
                        aliases.remove(target.id.as_str());
                    }
                    if let Expr::Name(source) = assignment.value.as_ref()
                        && (source.id == "__all__" || all_aliases.contains(source.id.as_str()))
                    {
                        all_aliases.insert(target.id.to_string());
                    }
                }
            }
            Stmt::AnnAssign(assignment) => {
                if let Expr::Name(target) = assignment.target.as_ref() {
                    if target.id != "__all__" {
                        all_aliases.remove(target.id.as_str());
                    }
                    let count = assignment
                        .value
                        .as_deref()
                        .map_or(0, |value| count_export_names(value, name, &aliases));
                    if count > 0 {
                        aliases.insert(target.id.to_string(), count);
                    } else {
                        aliases.remove(target.id.as_str());
                    }
                    if let Some(Expr::Name(source)) = assignment.value.as_deref()
                        && (source.id == "__all__" || all_aliases.contains(source.id.as_str()))
                    {
                        all_aliases.insert(target.id.to_string());
                    }
                }
            }
            Stmt::AugAssign(assignment) => {
                if let Expr::Name(target) = assignment.target.as_ref() {
                    if assignment.op == Operator::Add && target.id != "__all__" {
                        let count = count_export_names(&assignment.value, name, &aliases);
                        *aliases.entry(target.id.to_string()).or_default() += count;
                        if all_aliases.contains(target.id.as_str()) {
                            export_count += count;
                        }
                    } else {
                        aliases.remove(target.id.as_str());
                    }
                }
            }
            _ => {}
        }

        match statement {
            Stmt::Assign(assignment) if assignment.targets.iter().any(is_all_target) => {
                all_aliases.clear();
                if let Expr::Name(alias) = assignment.value.as_ref() {
                    all_aliases.insert(alias.id.to_string());
                }
                export_count = count_export_names(&assignment.value, name, &aliases);
            }
            Stmt::AnnAssign(assignment) if is_all_target(&assignment.target) => {
                if let Some(value) = &assignment.value {
                    all_aliases.clear();
                    if let Expr::Name(alias) = value.as_ref() {
                        all_aliases.insert(alias.id.to_string());
                    }
                    export_count = count_export_names(value, name, &aliases);
                }
            }
            Stmt::AugAssign(assignment)
                if assignment.op == Operator::Add && is_all_target(&assignment.target) =>
            {
                export_count += count_export_names(&assignment.value, name, &aliases);
            }
            Stmt::Delete(delete) if delete.targets.iter().any(is_all_target) => {
                all_aliases.clear();
                export_count = 0;
            }
            _ => {}
        }
        let Stmt::Expr(expression) = statement else {
            continue;
        };
        let Expr::Call(call) = expression.value.as_ref() else {
            continue;
        };
        let Expr::Attribute(attribute) = call.func.as_ref() else {
            continue;
        };
        let Expr::Name(receiver) = attribute.value.as_ref() else {
            continue;
        };
        let argument_count = call
            .arguments
            .args
            .iter()
            .map(|argument| count_export_names(argument, name, &aliases))
            .sum::<usize>();
        if receiver.id != "__all__" {
            let alias_count = aliases.entry(receiver.id.to_string()).or_default();
            match attribute.attr.as_str() {
                "append" | "extend" | "insert" => *alias_count += argument_count,
                "clear" => *alias_count = 0,
                "remove" if argument_count > 0 => {
                    *alias_count = alias_count.saturating_sub(1);
                }
                _ => {}
            }
        }
        if receiver.id != "__all__" && !all_aliases.contains(receiver.id.as_str()) {
            continue;
        }
        match attribute.attr.as_str() {
            "append" | "extend" | "insert" => export_count += argument_count,
            "clear" => export_count = 0,
            "remove" if argument_count > 0 => export_count = export_count.saturating_sub(1),
            _ => {}
        }
    }
    export_count > 0
}

fn is_all_target(expression: &Expr) -> bool {
    matches!(expression, Expr::Name(name) if name.id == "__all__")
}

fn has_deferred_annotations(statements: &[Stmt]) -> bool {
    statements.iter().any(|statement| {
        let Stmt::ImportFrom(import) = statement else {
            return false;
        };
        import
            .module
            .as_ref()
            .is_some_and(|module| module == "__future__")
            && import
                .names
                .iter()
                .any(|alias| alias.name.as_str() == "annotations")
    })
}

fn count_export_names(expression: &Expr, name: &str, aliases: &HashMap<String, usize>) -> usize {
    let mut visitor = StaticStringVisitor {
        name,
        aliases,
        count: 0,
    };
    visitor.visit_expr(expression);
    visitor.count
}

struct StaticStringVisitor<'a> {
    name: &'a str,
    aliases: &'a HashMap<String, usize>,
    count: usize,
}

impl<'a> Visitor<'a> for StaticStringVisitor<'_> {
    fn visit_expr(&mut self, expression: &'a Expr) {
        if get_static_string(expression).is_some_and(|value| value == self.name) {
            self.count += 1;
            return;
        }
        if let Expr::Name(alias) = expression
            && let Some(count) = self.aliases.get(alias.id.as_str())
        {
            self.count += count;
            return;
        }
        walk_expr(self, expression);
    }
}

fn does_function_bind_name(definition: &StmtFunctionDef, name: &str) -> bool {
    if definition
        .parameters
        .iter()
        .any(|parameter| parameter.name().as_str() == name)
    {
        return true;
    }
    let mut visitor = FunctionBindingVisitor {
        name,
        is_bound: false,
    };
    visitor.visit_body(&definition.body);
    visitor.is_bound
}

fn does_function_rebind_global_name(definition: &StmtFunctionDef, name: &str) -> bool {
    does_body_rebind_global_name(&definition.body, name)
}

fn does_body_rebind_global_name(body: &[Stmt], name: &str) -> bool {
    let mut declaration = GlobalDeclarationVisitor {
        name,
        is_declared: false,
    };
    declaration.visit_body(body);
    if !declaration.is_declared {
        return false;
    }

    let mut binding = FunctionBindingVisitor {
        name,
        is_bound: false,
    };
    binding.visit_body(body);
    binding.is_bound
}

struct GlobalDeclarationVisitor<'a> {
    name: &'a str,
    is_declared: bool,
}

impl<'a> Visitor<'a> for GlobalDeclarationVisitor<'a> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        match statement {
            Stmt::Global(global) => {
                self.is_declared |= global.names.iter().any(|name| name == self.name);
            }
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            _ => walk_stmt(self, statement),
        }
    }
}

struct FunctionBindingVisitor<'a> {
    name: &'a str,
    is_bound: bool,
}

impl<'a> FunctionBindingVisitor<'a> {
    fn visit_comprehension(&mut self, generators: &'a [Comprehension], elements: &[&'a Expr]) {
        for generator in generators {
            self.visit_expr(&generator.iter);
            for condition in &generator.ifs {
                self.visit_expr(condition);
            }
        }
        for element in elements {
            self.visit_expr(element);
        }
    }
}

impl<'a> Visitor<'a> for FunctionBindingVisitor<'a> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        match statement {
            Stmt::FunctionDef(definition) => {
                self.is_bound |= definition.name.as_str() == self.name;
            }
            Stmt::ClassDef(definition) => {
                self.is_bound |= definition.name.as_str() == self.name;
            }
            Stmt::Global(_) => {}
            Stmt::Nonlocal(nonlocal) => {
                self.is_bound |= nonlocal.names.iter().any(|name| name == self.name);
            }
            _ => walk_stmt(self, statement),
        }
    }

    fn visit_expr(&mut self, expression: &'a Expr) {
        if let Expr::Lambda(lambda) = expression {
            if let Some(parameters) = &lambda.parameters {
                for parameter in parameters.iter() {
                    if let Some(default) = parameter.default() {
                        self.visit_expr(default);
                    }
                }
            }
            return;
        }
        match expression {
            Expr::ListComp(comprehension) => {
                self.visit_comprehension(&comprehension.generators, &[&comprehension.elt]);
                return;
            }
            Expr::SetComp(comprehension) => {
                self.visit_comprehension(&comprehension.generators, &[&comprehension.elt]);
                return;
            }
            Expr::Generator(comprehension) => {
                self.visit_comprehension(&comprehension.generators, &[&comprehension.elt]);
                return;
            }
            Expr::DictComp(comprehension) => {
                if let Some(key) = &comprehension.key {
                    self.visit_comprehension(
                        &comprehension.generators,
                        &[key, &comprehension.value],
                    );
                } else {
                    self.visit_comprehension(&comprehension.generators, &[&comprehension.value]);
                }
                return;
            }
            _ => {}
        }
        if let Expr::Name(name) = expression
            && name.ctx == ExprContext::Store
            && name.id == self.name
        {
            self.is_bound = true;
            return;
        }
        walk_expr(self, expression);
    }

    fn visit_alias(&mut self, alias: &'a Alias) {
        let bound_name = get_bound_alias_name(alias);
        self.is_bound |= bound_name == self.name;
    }

    fn visit_except_handler(&mut self, handler: &'a ExceptHandler) {
        self.is_bound |= does_exception_bind_name(handler, self.name);
        walk_except_handler(self, handler);
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        self.is_bound |= does_pattern_bind_name(pattern, self.name);
        walk_pattern(self, pattern);
    }
}

fn does_statement_bind_name(statement: &Stmt, name: &str) -> bool {
    if matches!(statement, Stmt::AnnAssign(assignment) if assignment.value.is_none()) {
        return false;
    }
    let mut visitor = BindingVisitor {
        name,
        is_bound: false,
    };
    visitor.visit_stmt(statement);
    visitor.is_bound
}

fn does_statement_delete_name(statement: &Stmt, name: &str) -> bool {
    let Stmt::Delete(delete) = statement else {
        return false;
    };
    delete
        .targets
        .iter()
        .any(|target| does_expression_delete_name(target, name))
}

fn does_expression_delete_name(expression: &Expr, name: &str) -> bool {
    let mut visitor = DeletedNameVisitor {
        name,
        is_deleted: false,
    };
    visitor.visit_expr(expression);
    visitor.is_deleted
}

struct DeletedNameVisitor<'a> {
    name: &'a str,
    is_deleted: bool,
}

impl<'a> Visitor<'a> for DeletedNameVisitor<'a> {
    fn visit_expr(&mut self, expression: &'a Expr) {
        if let Expr::Name(deleted) = expression
            && deleted.ctx == ExprContext::Del
            && deleted.id == self.name
        {
            self.is_deleted = true;
            return;
        }
        walk_expr(self, expression);
    }
}

fn does_expression_bind_name(expression: &Expr, name: &str) -> bool {
    let mut visitor = BindingVisitor {
        name,
        is_bound: false,
    };
    visitor.visit_expr(expression);
    visitor.is_bound
}

fn does_pattern_recursively_bind_name(pattern: &Pattern, name: &str) -> bool {
    let mut visitor = BindingVisitor {
        name,
        is_bound: false,
    };
    visitor.visit_pattern(pattern);
    visitor.is_bound
}

struct BindingVisitor<'a> {
    name: &'a str,
    is_bound: bool,
}

impl<'a> BindingVisitor<'a> {
    fn visit_comprehension(&mut self, generators: &'a [Comprehension], elements: &[&'a Expr]) {
        for generator in generators {
            self.visit_expr(&generator.iter);
            for condition in &generator.ifs {
                self.visit_expr(condition);
            }
        }
        for element in elements {
            self.visit_expr(element);
        }
    }
}

impl<'a> Visitor<'a> for BindingVisitor<'a> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        match statement {
            Stmt::FunctionDef(definition) => {
                self.is_bound |= definition.name.as_str() == self.name;
            }
            Stmt::ClassDef(definition) => {
                self.is_bound |= definition.name.as_str() == self.name;
            }
            _ => walk_stmt(self, statement),
        }
    }

    fn visit_expr(&mut self, expression: &'a Expr) {
        if let Expr::Lambda(lambda) = expression {
            if let Some(parameters) = &lambda.parameters {
                for parameter in parameters.iter() {
                    if let Some(default) = parameter.default() {
                        self.visit_expr(default);
                    }
                }
            }
            return;
        }
        match expression {
            Expr::ListComp(comprehension) => {
                self.visit_comprehension(&comprehension.generators, &[&comprehension.elt]);
                return;
            }
            Expr::SetComp(comprehension) => {
                self.visit_comprehension(&comprehension.generators, &[&comprehension.elt]);
                return;
            }
            Expr::Generator(comprehension) => {
                self.visit_comprehension(&comprehension.generators, &[&comprehension.elt]);
                return;
            }
            Expr::DictComp(comprehension) => {
                if let Some(key) = &comprehension.key {
                    self.visit_comprehension(
                        &comprehension.generators,
                        &[key, &comprehension.value],
                    );
                } else {
                    self.visit_comprehension(&comprehension.generators, &[&comprehension.value]);
                }
                return;
            }
            _ => {}
        }
        if let Expr::Name(name) = expression
            && name.ctx == ExprContext::Store
            && name.id == self.name
        {
            self.is_bound = true;
            return;
        }
        walk_expr(self, expression);
    }

    fn visit_alias(&mut self, alias: &'a Alias) {
        let bound_name = get_bound_alias_name(alias);
        self.is_bound |= bound_name == self.name;
    }

    fn visit_except_handler(&mut self, handler: &'a ExceptHandler) {
        walk_except_handler(self, handler);
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        self.is_bound |= does_pattern_bind_name(pattern, self.name);
        walk_pattern(self, pattern);
    }
}

fn does_exception_bind_name(handler: &ExceptHandler, name: &str) -> bool {
    let ExceptHandler::ExceptHandler(handler) = handler;
    handler.name.as_ref().is_some_and(|bound| bound == name)
}

fn does_handler_body_bind_name(handler: &ExceptHandler, name: &str) -> bool {
    let ExceptHandler::ExceptHandler(handler) = handler;
    handler
        .body
        .iter()
        .any(|statement| does_statement_bind_name(statement, name))
}

fn does_pattern_bind_name(pattern: &Pattern, name: &str) -> bool {
    match pattern {
        Pattern::MatchAs(pattern) => pattern.name.as_ref().is_some_and(|bound| bound == name),
        Pattern::MatchStar(pattern) => pattern.name.as_ref().is_some_and(|bound| bound == name),
        Pattern::MatchMapping(pattern) => pattern.rest.as_ref().is_some_and(|bound| bound == name),
        _ => false,
    }
}

fn make_finding(
    path: &Path,
    code: impl Into<String>,
    message: impl Into<String>,
    range: TextRange,
    source: &str,
    caller_range: Option<TextRange>,
    noqa_row: Option<usize>,
) -> Finding {
    let location = locate_offset(source, range.start().to_usize());
    let end_location = locate_offset(source, range.end().to_usize());
    let source_line = source
        .lines()
        .nth(location.row.saturating_sub(1))
        .unwrap_or_default()
        .to_string();
    let caller = caller_range.map(|range| locate_offset(source, range.start().to_usize()));

    Finding {
        path: path.to_path_buf(),
        code: code.into(),
        message: message.into(),
        location,
        end_location,
        noqa_row,
        source_line,
        caller,
    }
}

fn locate_offset(source: &str, offset: usize) -> Location {
    let prefix = &source[..offset.min(source.len())];
    let row = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let column = prefix[line_start..].chars().count() + 1;
    Location { row, column }
}

fn has_noqa(source: &str, tokens: &Tokens, row: usize, code: &str) -> bool {
    tokens.iter().any(|token| {
        token.kind() == TokenKind::Comment
            && locate_offset(source, token.start().to_usize()).row == row
            && is_noqa_directive(&source[token.range()], code)
    })
}

fn find_noqa_row(source: &str, tokens: &Tokens, range: TextRange) -> usize {
    tokens
        .iter()
        .skip_while(|token| token.end() <= range.start())
        .find(|token| token.kind() == TokenKind::Newline)
        .map_or_else(
            || locate_offset(source, range.start().to_usize()).row,
            |token| locate_offset(source, token.start().to_usize()).row,
        )
}

fn is_noqa_directive(comment: &str, code: &str) -> bool {
    let lowercase = comment.to_ascii_lowercase();
    let Some(directive) = lowercase
        .strip_prefix('#')
        .and_then(|directive| directive.trim_start().strip_prefix("noqa"))
    else {
        return false;
    };
    if directive
        .chars()
        .next()
        .is_some_and(|character| character != ':' && !character.is_ascii_whitespace())
    {
        return false;
    }
    let directive = directive.trim_start();
    let Some(rules) = directive.strip_prefix(':') else {
        return true;
    };
    rules
        .split(|character: char| character == ',' || character.is_ascii_whitespace())
        .any(|rule| rule.eq_ignore_ascii_case(code))
}

fn print_findings(findings: &[Finding], output_format: OutputFormat) -> Result<bool, RunError> {
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let result = match output_format {
        OutputFormat::Full => print_full(&mut writer, findings),
        OutputFormat::Concise => print_concise(&mut writer, findings),
        OutputFormat::Json => print_json(&mut writer, findings),
        OutputFormat::Github => print_github(&mut writer, findings),
    };
    handle_output_result(result)
}

fn handle_output_result(result: io::Result<()>) -> Result<bool, RunError> {
    match result {
        Ok(()) => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(true),
        Err(error) => Err(RunError(format!("Failed to write output: {error}"))),
    }
}

fn print_full(writer: &mut impl Write, findings: &[Finding]) -> io::Result<()> {
    for finding in findings {
        print_concise_finding(writer, finding)?;
        writeln!(writer, "  |")?;
        writeln!(writer, "{} | {}", finding.location.row, finding.source_line)?;
        let padding = " ".repeat(finding.location.column.saturating_sub(1));
        let width = finding
            .end_location
            .column
            .saturating_sub(finding.location.column)
            .max(1);
        writeln!(
            writer,
            "  | {padding}{} {}",
            "^".repeat(width),
            finding.code
        )?;
        if let Some(caller) = &finding.caller {
            writeln!(
                writer,
                "  = note: Sole caller at {}:{}:{}",
                finding.path.display(),
                caller.row,
                caller.column
            )?;
        }
        writeln!(writer)?;
    }
    print_summary(writer, findings)
}

fn print_concise(writer: &mut impl Write, findings: &[Finding]) -> io::Result<()> {
    for finding in findings {
        print_concise_finding(writer, finding)?;
    }
    print_summary(writer, findings)
}

fn print_concise_finding(writer: &mut impl Write, finding: &Finding) -> io::Result<()> {
    writeln!(
        writer,
        "{}:{}:{}: {} {}",
        finding.path.display(),
        finding.location.row,
        finding.location.column,
        finding.code,
        finding.message,
    )
}

fn print_summary(writer: &mut impl Write, findings: &[Finding]) -> io::Result<()> {
    if findings.is_empty() {
        return writeln!(writer, "All checks passed!");
    }

    let suffix = if findings.len() == 1 { "" } else { "s" };
    writeln!(writer, "Found {} error{suffix}.", findings.len())
}

fn print_json(writer: &mut impl Write, findings: &[Finding]) -> io::Result<()> {
    let findings: Vec<_> = findings
        .iter()
        .map(|finding| JsonFinding {
            cell: None,
            code: &finding.code,
            end_location: &finding.end_location,
            filename: make_absolute_path(&finding.path).display().to_string(),
            fix: None,
            location: &finding.location,
            message: &finding.message,
            name: get_finding_name(&finding.code),
            noqa_row: finding.noqa_row,
            severity: "error",
            url: None,
        })
        .collect();
    let json = serde_json::to_string_pretty(&findings).map_err(io::Error::other)?;
    writeln!(writer, "{json}")
}

fn get_finding_name(code: &str) -> &str {
    match code {
        PRIVATE_CALL_WRAPPER_CODE => "private-call-wrapper",
        EXPLICIT_PRIVATE_INPUTS_CODE => "explicit-private-inputs",
        _ => "invalid-syntax",
    }
}

fn print_github(writer: &mut impl Write, findings: &[Finding]) -> io::Result<()> {
    for finding in findings {
        let absolute_path = make_absolute_path(&finding.path);
        let path = absolute_path.display();
        if finding.location.row == finding.end_location.row {
            write!(
                writer,
                "::error title=Ruffhouse ({}),file={},line={},col={},endLine={},endColumn={}::",
                finding.code,
                path,
                finding.location.row,
                finding.location.column,
                finding.end_location.row,
                finding.end_location.column
            )?;
        } else {
            write!(
                writer,
                "::error title=Ruffhouse ({}),file={},line={},endLine={}::",
                finding.code, path, finding.location.row, finding.end_location.row
            )?;
        }
        write!(
            writer,
            "{}:{}:{}: {} {}",
            path, finding.location.row, finding.location.column, finding.code, finding.message
        )?;
        if let Some(caller) = &finding.caller {
            write!(
                writer,
                "%0A  {}:{}:{}: Sole caller",
                path, caller.row, caller.column
            )?;
        }
        writeln!(writer)?;
    }
    Ok(())
}

fn make_absolute_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_or_else(|_| path.to_path_buf(), |current| current.join(path))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_source(source: &str) -> Vec<Finding> {
        super::check_source(Path::new("test.py"), source)
            .into_iter()
            .filter(|finding| finding.code == PRIVATE_CALL_WRAPPER_CODE)
            .collect()
    }

    fn check_explicit_private_inputs(source: &str) -> Vec<Finding> {
        super::check_source(Path::new("test.py"), source)
            .into_iter()
            .filter(|finding| finding.code == EXPLICIT_PRIVATE_INPUTS_CODE)
            .collect()
    }

    #[test]
    fn flags_private_definition_with_positional_input() {
        let findings = check_explicit_private_inputs("def _render(path):\n    ...\n");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "RH002");
        assert_eq!(
            findings[0].message,
            "Private definition `_render` must receive required keyword-only inputs."
        );
        assert_eq!(findings[0].location.row, 1);
        assert_eq!(findings[0].location.column, 5);
    }

    #[test]
    fn flags_private_method_with_positional_input_after_receiver() {
        let findings = check_explicit_private_inputs(
            "class Renderer:\n    def _render(self, path):\n        ...\n",
        );

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "RH002");
        assert_eq!(findings[0].location.row, 2);
        assert_eq!(findings[0].location.column, 9);
    }

    #[test]
    fn checks_explicit_private_input_signature_shapes() {
        let violations = [
            "def _load(path, /):\n    ...\n",
            "def _load(*, path=None):\n    ...\n",
            "async def _load(path):\n    ...\n",
            "def _load(path):\n    yield path\n",
            "@boundary\ndef _load(path):\n    ...\n",
            "class Service:\n    @staticmethod\n    def _load(path):\n        ...\n",
            "class Service:\n    @classmethod\n    def _load(cls, path):\n        ...\n",
            "class Service:\n    @property\n    def _value(self):\n        ...\n\n    @_value.setter\n    def _value(self, value):\n        ...\n",
            "def build():\n    class Service:\n        def _load(self, path):\n            ...\n",
        ];
        for source in violations {
            assert_eq!(
                check_explicit_private_inputs(source).len(),
                1,
                "expected RH002 for {source}"
            );
        }

        let allowed = [
            "def load(path):\n    ...\n",
            "def _load():\n    ...\n",
            "def _load(*, path):\n    ...\n",
            "def _forward(*args, **kwargs):\n    ...\n",
            "def outer():\n    def _load(path):\n        ...\n",
            "def _():\n    ...\n",
            "def __load(path):\n    ...\n",
            "def _missing_(path):\n    ...\n",
            "class Service:\n    def _load(self, *, path):\n        ...\n",
            "class Service:\n    @classmethod\n    def _load(cls, *, path):\n        ...\n",
            "class Service:\n    def _load(self=None):\n        ...\n",
            "def _load(path):  # noqa: RH002\n    ...\n",
        ];
        for source in allowed {
            assert!(
                check_explicit_private_inputs(source).is_empty(),
                "unexpected RH002 for {source}"
            );
        }
    }

    #[test]
    fn flags_direct_private_call_wrapper() {
        let findings = check_source(
            "def _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
        );

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].location.row, 1);
        assert_eq!(findings[0].caller.as_ref().unwrap().row, 5);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nasync def run(path):\n    return await _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):  # noqaish\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(\\\n    path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings[0].noqa_row, Some(2));

        let findings = check_source(
            "def _load(path: \"# noqa: RH001 \"):\n    return load(path)\n\nclass Service:\n    def _load(self, path):\n        return other(path)\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    _load = 1\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    def _load(path):\n        return other(path)\n\n    def run(path):\n        return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def __load(path):\n    return load(path)\n\ndef run(path):\n    return __load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    _load = _load(\"path\")\n",
        );
        assert!(findings.is_empty());

        let findings = check_source(
            "LABEL = \"_load\"\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\ndef run(path):\n    client._load(path)\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    class _load:\n        pass\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\ndef collect(callbacks):\n    return [callback for _load in callbacks for callback in [_load]]\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\ndef run(callbacks, path):\n    values = [callback for _load in callbacks for callback in [_load]]\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    _load = callback\n    del _load\n    value = _load(\"path\")\n",
        );
        assert!(findings.is_empty());

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    try:\n        pass\n    except Exception as _load:\n        value = _load(\"fallback\")\n    value = _load(\"path\")\n",
        );
        assert!(findings.is_empty());

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    _load = callback\n    run = lambda path: _load(path)\n",
        );
        assert!(findings.is_empty());

        let findings = check_source(
            "_load = callback\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "from plugin import *\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    if enabled:\n        value = _load(\"path\")\n    else:\n        _load = callback\n",
        );
        assert!(findings.is_empty());

        let findings = check_source(
            "LOAD_NAME = \"_load\"\nLOAD_NAME = \"different\"\n__all__ = [LOAD_NAME]\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    try:\n        value = _load(\"path\")\n    except Exception:\n        _load = callback\n",
        );
        assert!(findings.is_empty());

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\ndef run(path):\n    callback = lambda: (_load := other)\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "__all__ = [\"_load\"]\n__all__ = []\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Outer:\n    _load = callback\n    class Inner:\n        value = _load(\"path\")\n",
        );
        assert!(findings.is_empty());

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    while _load(\"path\"):\n        _load = callback\n",
        );
        assert!(findings.is_empty());

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\ncallback = lambda path: ((lambda: (_load := other))(), _load(path))[1]\n",
        );
        assert!(findings.is_empty());

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    callback = lambda: (_load := other)\n    value = _load(\"path\")\n",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn flags_wrapper_with_call_free_binding() {
        let findings = check_source(
            "def _paint(rgb):\n    color = rgb[:3]\n    draw(color=color)\n\ndef run(rgb):\n    _paint(rgb)\n",
        );

        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    _load = load\n    return _load(path)\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(_load):\n    return _load()\n\ndef run(callback):\n    return _load(callback)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "items = (_load(path) for path in paths)\n\ndef _load(path):\n    return load(path)\n\nconsume(items)\n",
        );
        assert!(findings.is_empty());

        let findings = check_source(
            "__all__ = [\"_load\"]\n__all__.clear()\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
        );
        assert_eq!(findings.len(), 1);

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    _load: object\n    value = _load(\"path\")\n",
        );
        assert!(findings.is_empty());

        let findings = check_source(
            "def _load(path):\n    return load(path)\n\nclass Service:\n    values = [value for _load in callbacks for value in [_load]]\n    value = _load(\"path\")\n",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn excludes_meaningful_or_reused_functions() {
        let cases = [
            "def _load(path):\n    if path:\n        return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "def _load(path):\n    return normalize(load(path))\n\ndef run(path):\n    return _load(path)\n",
            "def _load(paths):\n    return load(path for path in paths)\n\ndef run(paths):\n    return _load(paths)\n",
            "def _load(path):\n    return load(path)\n\ndef run(path):\n    consume(_load)\n",
            "def _load(path):\n    return load(path)\n\ndef run(path):\n    _load(path)\n    _load(path)\n",
            "@boundary\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "def _load(path):\n    return load(path)\n\ndef run(path):\n    def _load(path):\n        return other(path)\n    return _load(path)\n",
            "def _load(path):\n    return _load(path)\n",
            "def _load(path):\n    local = path\n    return _load(local)\n",
            "def _load(path):\n    return load(local := path)\n\ndef run(path):\n    return _load(path)\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    def _load(path):\n        return other(path)\n    value = _load(\"value\")\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    def run(_load, path):\n        return _load(path)\n",
            "def _load(path):\n    return load(path)\n\ndef run():\n    try:\n        pass\n    except Exception as _load:\n        return _load(\"value\")\n",
            "def _load(path):\n    return load(path)\n\ndef run(value):\n    match value:\n        case {\"loader\": _load}:\n            return _load(\"value\")\n",
            "def _load(path):\n    return load(path)\n\ndef make_value[_load](value: _load(\"path\")):\n    return value\n",
            "def _load(path):\n    return load(path)\n\nclass Value[_load]:\n    value = _load(\"path\")\n",
            "def _load(path):\n    return load(path)\n\ntype Value[_load] = _load(\"path\")\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    def run(self, callback=lambda _load: _load(\"path\")):\n        return callback()\n",
            "def _decorate(*args):\n    return make_decorator(*args)\n\n@_decorate(\"value\")\ndef run():\n    pass\n",
            "__all__ = [f\"_load\"]\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "__all__ = [\"_lo\" + \"ad\"]\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "__all__ = []\n__all__.append(\"_load\")\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "LOAD_NAME = \"_load\"\n__all__ = [LOAD_NAME]\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "def _load(path=_load(\"default\")):\n    return load(path)\n",
            "def _load(path):\n    return load(path)\n\ndef run():\n    import _load.plugin\n    return _load(\"path\")\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    values = [_load(path) for _load, path in callbacks]\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    for _load in callbacks:\n        value = _load(\"path\")\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    with manager() as _load:\n        value = _load(\"path\")\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    try:\n        pass\n    except Exception as _load:\n        value = _load(\"path\")\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    match value:\n        case {\"loader\": _load}:\n            result = _load(\"path\")\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    if enabled:\n        _load = callback\n        value = _load(\"path\")\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    for item in values:\n        _load = callback\n        value = _load(\"path\")\n",
            "def _load(path):\n    return load(path)\n\ndef outer(paths):\n    _load = callback\n    class Service:\n        values = [_load(path) for path in paths]\n    return Service\n",
            "def _load(path):\n    return load(path)\n\nvalues = [[_load(path) for path in paths] for _load in callbacks]\n",
            "def _load(path):\n    return load(path)\n\nvalues = [(lambda path: _load(path)) for _load in callbacks]\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    if (_load := callback):\n        value = _load(\"path\")\n",
            "def _load(path):\n    return load(path)\n\ndef replace():\n    global _load\n    _load = replacement\n\ndef run(path):\n    return _load(path)\n",
            "values = [_load(value) for value in [1]]\n\ndef _load(path):\n    return load(path)\n",
            "def _load(path):\n    return load(path)\n\nvalues = [(lambda: _load()) for item in items for _load in callbacks]\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    for item in items:\n        _load = replacement\n    else:\n        value = _load(\"path\")\n",
            "__all__ = [\"_load\", \"_load\"]\n__all__.remove(\"_load\")\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "EXPORTS = [\"_load\"]\n__all__ = EXPORTS\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "def _load(path):  #noqa: RH001\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    global _load\n    _load = replacement\n\ndef run(path):\n    return _load(path)\n",
            "def _load(path):\n    return load(path)\n\nclass Service:\n    _load += replacement\n",
            "EXPORTS = []\n__all__ = EXPORTS\nEXPORTS.append(\"_load\")\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "__all__ = []\nexports = __all__\nexports.append(\"_load\")\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "__all__ = []\n__all__.insert(0, \"_load\")\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "__all__ = []\nexports = __all__\nexports += [\"_load\"]\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "from __future__ import annotations\n\ndef _load(path):\n    return load(path)\n\ndef run(value: _load(\"path\")):\n    return value\n",
            "type Loader = _load\n\ndef _load(path):\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
            "def _load(path):\n    return load(path)\n\ndef replace():\n    global _load\n    def _load(path):\n        return other(path)\n    return _load(\"path\")\n",
            "def _load(path):\n    return load(path)\n\nfrom plugin import *\n\ndef run(path):\n    return _load(path)\n",
            "def _load(\\\n    path):  # noqa: RH001\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
        ];

        for source in cases {
            assert!(
                check_source(source).is_empty(),
                "unexpected finding for {source}"
            );
        }

        let findings = check_source(
            "def _load(path):  # noqa: RH001\n    return load(path)\n\ndef run(path):\n    return _load(path)\n",
        );
        assert!(findings.is_empty());
    }

    #[test]
    fn handles_broken_output_pipes() {
        let error = io::Error::new(io::ErrorKind::BrokenPipe, "closed");

        assert!(handle_output_result(Err(error)).unwrap());
    }
}
