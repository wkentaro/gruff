use std::collections::BTreeMap;
use std::collections::HashMap;
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
use ruff_python_ast::Stmt;
use ruff_python_ast::token::TokenKind;
use ruff_python_ast::token::Tokens;
use ruff_python_parser::parse_module;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;
use serde::Deserialize;
use serde::Serialize;

mod analysis;
mod rules;

struct Rule {
    code: &'static str,
    name: &'static str,
    check: fn(&Path, &[Stmt]) -> Vec<rules::Diagnostic>,
}

const RULES: &[Rule] = &[
    Rule {
        code: rules::explicit_input_conventions::CODE,
        name: rules::explicit_input_conventions::NAME,
        check: rules::explicit_input_conventions::check,
    },
    Rule {
        code: rules::required_private_inputs::CODE,
        name: rules::required_private_inputs::NAME,
        check: rules::required_private_inputs::check,
    },
    Rule {
        code: rules::package_dunder_all::CODE,
        name: rules::package_dunder_all::NAME,
        check: rules::package_dunder_all::check,
    },
    Rule {
        code: rules::final_constants::CODE,
        name: rules::final_constants::NAME,
        check: rules::final_constants::check,
    },
];
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
#[command(name = "gruff", version, about)]
pub struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run Gruff on the given files or directories
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

    /// Output serialization format for findings
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
    gruff: Option<RawConfig>,
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

#[derive(Clone, Debug, PartialEq, Serialize)]
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
        for rule in RULES {
            let is_enabled = resolve_rule_enabled(&arguments, &config.raw, rule.code)?;
            is_rule_enabled_anywhere |= is_enabled;
            if !is_enabled || is_ignored_for_file(&path, &config, rule.code)? {
                file_findings.retain(|finding| finding.code != rule.code);
            }
        }
        findings.extend(file_findings);
    }

    if !has_files {
        for rule in RULES {
            is_rule_enabled_anywhere |=
                resolve_rule_enabled(&arguments, &base_config.raw, rule.code)?;
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
        if let Some(raw) = document.tool.gruff {
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
    let raw = document
        .tool
        .gruff
        .ok_or_else(|| RunError(format!("{} does not contain [tool.gruff]", path.display())))?;
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
        || (!selector.is_empty() && RULES.iter().any(|rule| rule.code.starts_with(selector)))
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
            return vec![make_finding(
                path,
                "invalid-syntax",
                error.error.to_string(),
                error.location,
                source,
                None,
            )];
        }
    };
    let mut findings = Vec::new();

    for rule in RULES {
        for diagnostic in (rule.check)(path, parsed.suite()) {
            let noqa_row = find_noqa_row(source, parsed.tokens(), diagnostic.range);
            if has_noqa(source, parsed.tokens(), noqa_row, rule.code) {
                continue;
            }
            findings.push(make_finding(
                path,
                rule.code,
                diagnostic.message,
                diagnostic.range,
                source,
                Some(noqa_row),
            ));
        }
    }

    findings
}

fn make_finding(
    path: &Path,
    code: impl Into<String>,
    message: impl Into<String>,
    range: TextRange,
    source: &str,
    noqa_row: Option<usize>,
) -> Finding {
    let location = locate_offset(source, range.start().to_usize());
    let end_location = locate_offset(source, range.end().to_usize());
    let source_line = source
        .lines()
        .nth(location.row.saturating_sub(1))
        .unwrap_or_default()
        .to_string();
    Finding {
        path: path.to_path_buf(),
        code: code.into(),
        message: message.into(),
        location,
        end_location,
        noqa_row,
        source_line,
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
    writeln!(writer, "Found {} finding{suffix}.", findings.len())
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
    RULES
        .iter()
        .find(|rule| rule.code == code)
        .map_or("invalid-syntax", |rule| rule.name)
}

fn print_github(writer: &mut impl Write, findings: &[Finding]) -> io::Result<()> {
    for finding in findings {
        let absolute_path = make_absolute_path(&finding.path);
        let path = absolute_path.display();
        if finding.location.row == finding.end_location.row {
            write!(
                writer,
                "::error title=Gruff ({}),file={},line={},col={},endLine={},endColumn={}::",
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
                "::error title=Gruff ({}),file={},line={},endLine={}::",
                finding.code, path, finding.location.row, finding.end_location.row
            )?;
        }
        write!(
            writer,
            "{}:{}:{}: {} {}",
            path, finding.location.row, finding.location.column, finding.code, finding.message
        )?;
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

    fn check_rule(source: &str, code: &str) -> Vec<Finding> {
        check_source(Path::new("test.py"), source)
            .into_iter()
            .filter(|finding| finding.code == code)
            .collect()
    }

    #[test]
    fn reports_each_private_input() {
        let findings = check_source(
            Path::new("test.py"),
            "def _resize(data, width=512, *, mode=\"fit\"):\n    ...\n",
        );

        assert_eq!(findings.len(), 4);
        assert_eq!(findings[0].code, "GR001");
        assert_eq!(
            findings[0].message,
            "Input `data` must be positional-only or keyword-only"
        );
        assert_eq!(
            findings[1].message,
            "Input `width` must be positional-only or keyword-only"
        );
        assert_eq!(findings[2].code, "GR002");
        assert_eq!(
            findings[2].message,
            "Private input `width` must be required"
        );
        assert_eq!(findings[3].message, "Private input `mode` must be required");
    }

    #[test]
    fn reports_both_rules_for_a_defaulted_positional_input() {
        let findings = check_source(Path::new("test.py"), "def _f(width=512):\n    ...\n");

        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].code, "GR001");
        assert_eq!(findings[1].code, "GR002");
        assert_eq!(findings[0].location.row, 1);
        assert_eq!(findings[0].location.column, 8);
        assert_eq!(findings[0].location, findings[1].location);
    }

    #[test]
    fn reports_only_positional_or_keyword_inputs() {
        let findings = check_rule("def _load(path, /, mode):\n    ...\n", "GR001");

        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].message,
            "Input `mode` must be positional-only or keyword-only"
        );
    }

    #[test]
    fn reports_inputs_on_public_and_special_definitions() {
        let conventions = [
            "def load(path):\n    ...\n",
            "class Service:\n    def __eq__(self, other):\n        ...\n",
            "class Service:\n    def __load(self, path):\n        ...\n",
            "class Service:\n    def _load_(self, path):\n        ...\n",
            "class Service:\n    @staticmethod\n    def load(path):\n        ...\n",
        ];
        for source in conventions {
            assert_eq!(
                check_rule(source, "GR001").len(),
                1,
                "expected GR001 for {source}"
            );
        }

        let allowed = [
            "class Service:\n    def __eq__(self, other, /):\n        ...\n",
            "def outer():\n    def load(path):\n        ...\n",
            "load = lambda path: path\n",
        ];
        for source in allowed {
            assert!(
                check_rule(source, "GR001").is_empty(),
                "unexpected GR001 for {source}"
            );
        }
    }

    #[test]
    fn keeps_required_private_inputs_private_only() {
        let outside_scope = [
            "def load(path=None):\n    ...\n",
            "def __load(path=None):\n    ...\n",
            "def _missing_(path=None):\n    ...\n",
        ];
        for source in outside_scope {
            assert!(
                check_rule(source, "GR002").is_empty(),
                "unexpected GR002 for {source}"
            );
        }
    }

    #[test]
    fn classifies_private_definition_signature_shapes() {
        let implicit_conventions = [
            "async def _load(path):\n    ...\n",
            "class Service:\n    def _load(self, path):\n        ...\n",
            "class Service:\n    @staticmethod\n    def _load(path):\n        ...\n",
            "class Service:\n    @classmethod\n    def _load(cls, path):\n        ...\n",
            "def build():\n    class Service:\n        def _load(self, path):\n            ...\n",
        ];
        for source in implicit_conventions {
            assert_eq!(
                check_rule(source, "GR001").len(),
                1,
                "expected GR001 for {source}"
            );
        }

        let required_violations = [
            "def _load(path=None):\n    ...\n",
            "def _load(*, path=None):\n    ...\n",
            "class Service:\n    def _load(self, *, path=None):\n        ...\n",
        ];
        for source in required_violations {
            assert_eq!(
                check_rule(source, "GR002").len(),
                1,
                "expected GR002 for {source}"
            );
        }

        let allowed = [
            "def _load():\n    ...\n",
            "def _load(*, path):\n    ...\n",
            "def _forward(*args, **kwargs):\n    ...\n",
            "def _load(path, /):\n    ...\n",
            "async def _load(path, /):\n    ...\n",
            "def outer():\n    def _load(path=None):\n        ...\n",
            "def _():\n    ...\n",
            "class Service:\n    def _load(self, *, path):\n        ...\n",
            "class Service:\n    @classmethod\n    def _load(cls, *, path):\n        ...\n",
            "class Service:\n    def _load(self, path, /):\n        ...\n",
            "class Service:\n    @classmethod\n    def _load(cls, path, /):\n        ...\n",
            "class Service:\n    @staticmethod\n    def _load(path, /):\n        ...\n",
            "class Service:\n    def _load(self=None):\n        ...\n",
        ];
        for source in allowed {
            assert!(
                check_source(Path::new("test.py"), source).is_empty(),
                "unexpected finding for {source}"
            );
        }
    }

    #[test]
    fn keeps_required_private_inputs_independent_for_positional_only_inputs() {
        let findings = check_source(Path::new("test.py"), "def _load(path=None, /):\n    ...\n");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "GR002");
    }

    #[test]
    fn suppresses_each_rule_independently() {
        let findings = check_source(
            Path::new("test.py"),
            "def _load(\\\n    path=None):  # noqa: GR001 -- positional protocol\n    ...\n",
        );

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, "GR002");
        assert_eq!(findings[0].noqa_row, Some(2));
    }

    #[test]
    fn handles_broken_output_pipes() {
        let error = io::Error::new(io::ErrorKind::BrokenPipe, "closed");

        assert!(handle_output_result(Err(error)).unwrap());
    }
}
