use std::path::Path;

use ruff_python_ast::Stmt;
use ruff_python_ast::StmtImportFrom;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_stmt;

use super::Diagnostic;
use crate::analysis::is_non_public_name;
use crate::analysis::is_test_file;
use crate::exports::find_initializer;
use crate::exports::find_packages;
use crate::exports::get_extension;
use crate::exports::imported_file;

pub(crate) const CODE: &str = "GR014";
pub(crate) const NAME: &str = "no-non-public-imports";
pub(crate) const SUMMARY: &str = "Non-public names stay inside the module that defines them.";

pub(crate) fn check(path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    if is_test_file(path) || path.file_name().is_some_and(|name| name == "conftest.py") {
        return Vec::new();
    }
    let Ok(path) = path.canonicalize() else {
        return Vec::new();
    };
    let extension = get_extension(&path);
    let import_root = find_packages(&path)
        .last()
        .and_then(|package| package.parent());
    let mut imports = Imports::default();
    imports.visit_body(statements);
    let mut diagnostics = Vec::new();
    for import in imports.0 {
        if !import
            .names
            .iter()
            .any(|alias| is_non_public_name(alias.name.as_str()))
        {
            continue;
        }
        let base = if import.level == 0 {
            import_root
        } else {
            path.ancestors().nth(import.level as usize)
        };
        let Some(base) = base else {
            continue;
        };
        let target = match &import.module {
            Some(module) => imported_file(base, module.as_str(), extension),
            None => find_initializer(base, extension),
        };
        // The resolver canonicalizes, so a symlink can lead outside the import root.
        let Some(target) = target.filter(|target| {
            *target != path
                && (import.level > 0 || import_root.is_some_and(|root| target.starts_with(root)))
        }) else {
            continue;
        };
        // A package's own submodules are importable by name; only a package initializer has them.
        let package = target
            .file_stem()
            .is_some_and(|stem| stem == "__init__")
            .then(|| target.parent())
            .flatten();
        let module = format!(
            "{}{}",
            ".".repeat(import.level as usize),
            import.module.as_ref().map_or("", |module| module.as_str())
        );
        for alias in &import.names {
            let name = alias.name.as_str();
            if !is_non_public_name(name)
                || package.is_some_and(|package| {
                    package.join(name).is_dir() || imported_file(package, name, extension).is_some()
                })
            {
                continue;
            }
            diagnostics.push(Diagnostic {
                message: format!(
                    "Non-public name `{name}` is imported from `{module}`; drop the underscore"
                ),
                range: alias.name.range,
                noqa_offset: None,
            });
        }
    }
    diagnostics
}

#[derive(Default)]
struct Imports<'a>(Vec<&'a StmtImportFrom>);

impl<'a> Visitor<'a> for Imports<'a> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        if let Stmt::ImportFrom(import) = statement {
            self.0.push(import);
        }
        walk_stmt(self, statement);
    }
}
