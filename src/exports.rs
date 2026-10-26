use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use ruff_python_ast::Alias;
use ruff_python_ast::Comprehension;
use ruff_python_ast::ExceptHandler;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprContext;
use ruff_python_ast::Pattern;
use ruff_python_ast::Stmt;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_except_handler;
use ruff_python_ast::visitor::walk_expr;
use ruff_python_ast::visitor::walk_pattern;
use ruff_python_ast::visitor::walk_stmt;
use ruff_python_parser::parse_module;

use crate::analysis::is_non_public_name;

pub(crate) fn public_reexports(path: &Path, statements: &[Stmt]) -> HashSet<String> {
    let Ok(path) = path.canonicalize() else {
        return HashSet::new();
    };
    let packages: Vec<_> = path
        .ancestors()
        .skip(1)
        .take_while(|directory| {
            directory.join("__init__.py").is_file() || directory.join("__init__.pyi").is_file()
        })
        .collect();
    if packages.is_empty() {
        return HashSet::new();
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("py");
    let mut exports = HashSet::new();
    // shortcut: follow direct ancestor-package imports; add forwarding-module resolution when needed.
    for (index, directory) in packages.iter().enumerate() {
        if packages[index..].iter().any(|package| {
            package
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_non_public_name)
        }) {
            continue;
        }
        let initializer = directory.join(format!("__init__.{extension}"));
        let initializer = if initializer.is_file() {
            initializer
        } else {
            directory.join("__init__.py")
        };
        let Ok(source) = fs::read_to_string(initializer) else {
            continue;
        };
        let Ok(parsed) = parse_module(&source) else {
            continue;
        };
        let mut bindings = Bindings::default();
        bindings.visit_body(parsed.suite());
        if bindings.dynamic_namespace || bindings.all_read {
            continue;
        }
        let manifest = literal_manifest(parsed.suite());
        if bindings.counts.contains_key("__all__")
            && (bindings.counts["__all__"] != 1 || manifest.is_none())
        {
            continue;
        }
        for statement in parsed.suite() {
            let Stmt::ImportFrom(import) = statement else {
                continue;
            };
            let Some(module) = &import.module else {
                continue;
            };
            let matches = if import.level == 0 {
                directory.ancestors().any(|base| {
                    imported_file(base, module.as_str(), extension).as_ref() == Some(&path)
                })
            } else {
                directory
                    .ancestors()
                    .nth((import.level - 1) as usize)
                    .is_some_and(|base| {
                        imported_file(base, module.as_str(), extension).as_ref() == Some(&path)
                    })
            };
            if !matches {
                continue;
            }
            for alias in &import.names {
                let bound = alias.asname.as_ref().unwrap_or(&alias.name).as_str();
                if alias.name.as_str() == "*" || bindings.counts.get(bound) != Some(&1) {
                    continue;
                }
                let exported = manifest
                    .as_ref()
                    .map_or_else(|| !bound.starts_with('_'), |names| names.contains(bound));
                if exported {
                    exports.insert(alias.name.to_string());
                }
            }
        }
    }
    if exports.is_empty() {
        return exports;
    }
    let mut definitions = Bindings::default();
    definitions.visit_body(statements);
    exports.retain(|name| {
        let count = definitions.counts.get(name).copied().unwrap_or(0);
        let defined = definitions.definitions.get(name).copied().unwrap_or(0);
        let overloads = definitions.overloads.get(name).copied().unwrap_or(0);
        !definitions.dynamic_namespace
            && count == defined
            && (defined == 1 || (overloads > 0 && defined <= overloads + 1))
    });
    exports
}

fn imported_file(base: &Path, module: &str, extension: &str) -> Option<PathBuf> {
    let mut path = base.to_path_buf();
    let mut parts = module.split('.').peekable();
    while let Some(part) = parts.next() {
        path.push(part);
        let initializer = [extension, "py", "pyi"]
            .into_iter()
            .map(|extension| path.join(format!("__init__.{extension}")))
            .find(|initializer| initializer.is_file());
        if let Some(initializer) = initializer {
            if parts.peek().is_none() {
                return initializer.canonicalize().ok();
            }
            continue;
        }
        let file = [extension, "py", "pyi"]
            .into_iter()
            .map(|extension| path.with_extension(extension))
            .find(|file| file.is_file());
        if let Some(file) = file {
            return if parts.peek().is_none() {
                file.canonicalize().ok()
            } else {
                None
            };
        }
        if !path.is_dir() {
            return None;
        }
    }
    None
}

fn literal_manifest(statements: &[Stmt]) -> Option<HashSet<&str>> {
    statements.iter().find_map(|statement| {
        let (target, value) = match statement {
            Stmt::Assign(assignment) if assignment.targets.len() == 1 => {
                (&assignment.targets[0], assignment.value.as_ref())
            }
            Stmt::AnnAssign(assignment) => {
                (assignment.target.as_ref(), assignment.value.as_deref()?)
            }
            _ => return None,
        };
        if !matches!(target, Expr::Name(name) if name.id == "__all__") {
            return None;
        }
        let elements = match value {
            Expr::List(list) => &list.elts,
            Expr::Tuple(tuple) => &tuple.elts,
            _ => return None,
        };
        elements
            .iter()
            .map(|element| match element {
                Expr::StringLiteral(literal) => Some(literal.value.to_str()),
                _ => None,
            })
            .collect()
    })
}

#[derive(Default)]
struct Bindings {
    counts: HashMap<String, usize>,
    definitions: HashMap<String, usize>,
    overloads: HashMap<String, usize>,
    all_read: bool,
    dynamic_namespace: bool,
    class_depth: usize,
    globals: HashSet<String>,
}

impl Bindings {
    fn bind(&mut self, name: &str) {
        if self.class_depth == 0 || self.globals.contains(name) {
            *self.counts.entry(name.to_owned()).or_default() += 1;
        }
    }

    fn read_manifest(&mut self, expression: &Expr) {
        if !matches!(expression, Expr::Name(name) if name.id == "__all__") {
            self.visit_expr(expression);
        }
    }
}

impl<'a> Visitor<'a> for Bindings {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        match statement {
            Stmt::FunctionDef(function) => {
                self.bind(function.name.as_str());
                if self.class_depth == 0 {
                    *self
                        .definitions
                        .entry(function.name.to_string())
                        .or_default() += 1;
                    if function
                        .decorator_list
                        .iter()
                        .any(|decorator| match &decorator.expression {
                            Expr::Name(name) => name.id == "overload",
                            Expr::Attribute(attribute) => attribute.attr.as_str() == "overload",
                            _ => false,
                        })
                    {
                        *self.overloads.entry(function.name.to_string()).or_default() += 1;
                    }
                }
                for decorator in &function.decorator_list {
                    self.visit_decorator(decorator);
                }
                self.visit_parameters(&function.parameters);
                if let Some(returns) = &function.returns {
                    self.visit_annotation(returns);
                }
                return;
            }
            Stmt::ClassDef(class) => {
                self.bind(class.name.as_str());
                if self.class_depth == 0 {
                    *self.definitions.entry(class.name.to_string()).or_default() += 1;
                }
                for decorator in &class.decorator_list {
                    self.visit_decorator(decorator);
                }
                if let Some(arguments) = &class.arguments {
                    self.visit_arguments(arguments);
                }
                self.class_depth += 1;
                let globals = std::mem::take(&mut self.globals);
                self.visit_body(&class.body);
                self.globals = globals;
                self.class_depth -= 1;
                return;
            }
            Stmt::Global(global) => {
                self.globals
                    .extend(global.names.iter().map(ToString::to_string));
            }
            Stmt::Assert(assertion) => {
                self.read_manifest(&assertion.test);
                if let Some(message) = &assertion.msg {
                    self.visit_expr(message);
                }
                return;
            }
            Stmt::AnnAssign(assignment) if assignment.value.is_none() => {
                self.visit_annotation(&assignment.annotation);
                return;
            }
            _ => {}
        }
        walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'a Expr) {
        if let Expr::Lambda(lambda) = expression {
            if let Some(parameters) = &lambda.parameters {
                self.visit_parameters(parameters);
            }
            return;
        }
        if let Expr::Compare(comparison) = expression {
            self.read_manifest(&comparison.left);
            for comparator in &comparison.comparators {
                self.read_manifest(comparator);
            }
            return;
        }
        if let Expr::Name(name) = expression {
            if name.ctx != ExprContext::Load {
                self.bind(name.id.as_str());
            } else {
                self.all_read |= name.id == "__all__";
                self.dynamic_namespace |= matches!(
                    name.id.as_str(),
                    "globals" | "locals" | "vars" | "eval" | "exec"
                );
            }
        }
        walk_expr(self, expression);
    }

    fn visit_comprehension(&mut self, comprehension: &'a Comprehension) {
        // Comprehension targets are local; assignment expressions still bind in the outer scope.
        self.visit_expr(&comprehension.iter);
        for condition in &comprehension.ifs {
            self.visit_expr(condition);
        }
    }

    fn visit_alias(&mut self, alias: &'a Alias) {
        self.bind(alias.asname.as_ref().map_or_else(
            || alias.name.split('.').next().unwrap_or_default(),
            |name| name.as_str(),
        ));
        self.dynamic_namespace |= alias.name.as_str() == "*";
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        let name = match pattern {
            Pattern::MatchAs(pattern) => pattern.name.as_ref(),
            Pattern::MatchStar(pattern) => pattern.name.as_ref(),
            Pattern::MatchMapping(pattern) => pattern.rest.as_ref(),
            _ => None,
        };
        if let Some(name) = name {
            self.bind(name.as_str());
        }
        walk_pattern(self, pattern);
    }

    fn visit_except_handler(&mut self, handler: &'a ExceptHandler) {
        let ExceptHandler::ExceptHandler(body) = handler;
        if let Some(name) = &body.name {
            self.bind(name.as_str());
        }
        walk_except_handler(self, handler);
    }
}
