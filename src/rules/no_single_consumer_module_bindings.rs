use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use ruff_python_ast::Alias;
use ruff_python_ast::ExceptHandler;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprContext;
use ruff_python_ast::ExprName;
use ruff_python_ast::Parameter;
use ruff_python_ast::Pattern;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtClassDef;
use ruff_python_ast::StmtFunctionDef;
use ruff_python_ast::TypeParam;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_except_handler;
use ruff_python_ast::visitor::walk_expr;
use ruff_python_ast::visitor::walk_parameter;
use ruff_python_ast::visitor::walk_pattern;
use ruff_python_ast::visitor::walk_stmt;
use ruff_python_ast::visitor::walk_type_param;

use super::Diagnostic;
use crate::analysis::is_non_public_name;

pub(crate) const CODE: &str = "GR011";
pub(crate) const NAME: &str = "no-single-consumer-module-bindings";
pub(crate) const SUMMARY: &str =
    "Non-public module bindings live in the one definition that reads them.";

// Names whose presence means the module namespace is read or written by means the lexical walk
// cannot see, so no reference count in the module can be trusted.
const DYNAMIC_NAMESPACE_ACCESS: &[&str] = &["eval", "exec", "globals", "vars"];

pub(crate) fn check(_path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    let candidates = find_candidates(statements);
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut walker = ModuleWalker {
        usage: candidates
            .iter()
            .map(|candidate| (candidate.id.as_str(), Usage::default()))
            .collect(),
        consumers: Vec::new(),
        current_consumer: None,
        exported: HashSet::new(),
        has_dynamic_namespace_access: false,
    };
    walker.walk_module(statements);
    if walker.has_dynamic_namespace_access {
        return Vec::new();
    }

    candidates
        .iter()
        .filter(|candidate| !walker.exported.contains(candidate.id.as_str()))
        .filter_map(|candidate| {
            let usage = &walker.usage[candidate.id.as_str()];
            let Reader::Consumer(consumer) = usage.reader else {
                return None;
            };
            (usage.binding_sites == 1).then(|| Diagnostic {
                message: format!(
                    "Non-public module binding `{}` is used only by `{}`; move it into that definition",
                    candidate.id, walker.consumers[consumer]
                ),
                range: candidate.range,
                // A multi-line value ends on a later row; the suppression belongs beside the name.
                noqa_offset: Some(candidate.range.start()),
            })
        })
        .collect()
}

fn find_candidates(statements: &[Stmt]) -> Vec<&ExprName> {
    statements
        .iter()
        .filter_map(|statement| match statement {
            Stmt::Assign(assignment) => match assignment.targets.as_slice() {
                [Expr::Name(name)] => Some((name, &assignment.value)),
                _ => None,
            },
            Stmt::AnnAssign(assignment) => match (assignment.target.as_ref(), &assignment.value) {
                (Expr::Name(name), Some(value)) => Some((name, value)),
                _ => None,
            },
            _ => None,
        })
        .filter(|(name, value)| {
            // A `__name` read inside a class body is mangled to `_Class__name`, so a method
            // never reads the module binding the walk would attribute to it.
            is_non_public_name(&name.id)
                && !name.id.starts_with("__")
                && !contains_call(value)
                && !is_empty_display(value)
        })
        .map(|(name, _)| name)
        .collect()
}

// An empty list or dict at module scope is an accumulator the consumer fills across calls;
// rebuilt inside the body it would start empty on every call.
fn is_empty_display(value: &Expr) -> bool {
    match value {
        Expr::List(list) => list.elts.is_empty(),
        Expr::Dict(dict) => dict.items.is_empty(),
        _ => false,
    }
}

// A called value may be memoising work its consumer would otherwise repeat, so moving it changes
// runtime cost rather than scope; the trial's every intentional module constant was one of these.
fn contains_call(value: &Expr) -> bool {
    struct CallFinder(bool);

    impl<'a> Visitor<'a> for CallFinder {
        fn visit_expr(&mut self, expression: &'a Expr) {
            if matches!(expression, Expr::Call(_)) {
                self.0 = true;
            } else {
                walk_expr(self, expression);
            }
        }
    }

    let mut finder = CallFinder(false);
    finder.visit_expr(value);
    finder.0
}

#[derive(Clone, Copy, Default)]
enum Reader {
    #[default]
    Nobody,
    Consumer(usize),
    // A read outside any consumer body, or reads from two consumers; the binding stays.
    Elsewhere,
}

#[derive(Default)]
struct Usage {
    binding_sites: usize,
    reader: Reader,
}

struct ModuleWalker<'a> {
    usage: HashMap<&'a str, Usage>,
    consumers: Vec<String>,
    current_consumer: Option<usize>,
    exported: HashSet<&'a str>,
    has_dynamic_namespace_access: bool,
}

impl<'a> ModuleWalker<'a> {
    // Only a module-level function or a method of a module-level class is a consumer, and only
    // its body is the usage site: decorators, defaults, and annotations evaluate at definition
    // time, before a binding moved into the body would exist.
    fn walk_module(&mut self, statements: &'a [Stmt]) {
        for statement in statements {
            match statement {
                Stmt::FunctionDef(definition) => {
                    self.walk_consumer(definition, definition.name.to_string());
                }
                Stmt::ClassDef(class) => self.walk_class(class),
                _ => self.visit_stmt(statement),
            }
        }
    }

    fn walk_class(&mut self, class: &'a StmtClassDef) {
        self.bind(&class.name);
        for decorator in &class.decorator_list {
            self.visit_decorator(decorator);
        }
        if let Some(type_params) = &class.type_params {
            self.visit_type_params(type_params);
        }
        if let Some(arguments) = &class.arguments {
            self.visit_arguments(arguments);
        }
        for statement in &class.body {
            match statement {
                Stmt::FunctionDef(definition) => {
                    self.walk_consumer(definition, format!("{}.{}", class.name, definition.name));
                }
                _ => self.visit_stmt(statement),
            }
        }
    }

    fn walk_consumer(&mut self, definition: &'a StmtFunctionDef, display_name: String) {
        self.bind(&definition.name);
        for decorator in &definition.decorator_list {
            self.visit_decorator(decorator);
        }
        if let Some(type_params) = &definition.type_params {
            self.visit_type_params(type_params);
        }
        self.visit_parameters(&definition.parameters);
        if let Some(returns) = &definition.returns {
            self.visit_annotation(returns);
        }

        self.consumers.push(display_name);
        self.current_consumer = Some(self.consumers.len() - 1);
        self.visit_body(&definition.body);
        self.current_consumer = None;
    }

    fn bind(&mut self, name: &str) {
        if let Some(usage) = self.usage.get_mut(name) {
            usage.binding_sites += 1;
        }
    }

    fn read(&mut self, name: &str) {
        if DYNAMIC_NAMESPACE_ACCESS.contains(&name) {
            self.has_dynamic_namespace_access = true;
        }
        let Some(usage) = self.usage.get_mut(name) else {
            return;
        };
        usage.reader = match (usage.reader, self.current_consumer) {
            (Reader::Nobody, Some(consumer)) => Reader::Consumer(consumer),
            (Reader::Consumer(previous), Some(consumer)) if previous == consumer => usage.reader,
            _ => Reader::Elsewhere,
        };
    }

    // The stored-into target may be nested, as in `_cache["a"][key] = value`; the binding at the
    // base of the chain is the one whose state the store changes.
    fn record_mutation(&mut self, target: &Expr) {
        let mut base = target;
        loop {
            match base {
                Expr::Subscript(subscript) => base = &subscript.value,
                Expr::Attribute(attribute) => base = &attribute.value,
                Expr::Name(name) => {
                    if let Some(usage) = self.usage.get_mut(name.id.as_str()) {
                        usage.reader = Reader::Elsewhere;
                    }
                    return;
                }
                _ => return,
            }
        }
    }

    fn record_exports(&mut self, statement: &'a Stmt) {
        let (target, value) = match statement {
            Stmt::Assign(assignment) => (assignment.targets.first(), Some(&*assignment.value)),
            Stmt::AnnAssign(assignment) => (Some(&*assignment.target), assignment.value.as_deref()),
            Stmt::AugAssign(assignment) => (Some(&*assignment.target), Some(&*assignment.value)),
            _ => return,
        };
        if let (Some(Expr::Name(name)), Some(value)) = (target, value)
            && name.id == "__all__"
        {
            let mut collector = StringCollector(&mut self.exported);
            collector.visit_expr(value);
        }
    }
}

struct StringCollector<'a, 'b>(&'b mut HashSet<&'a str>);

impl<'a> Visitor<'a> for StringCollector<'a, '_> {
    fn visit_expr(&mut self, expression: &'a Expr) {
        if let Expr::StringLiteral(literal) = expression {
            self.0.insert(literal.value.to_str());
        }
        walk_expr(self, expression);
    }
}

impl<'a> Visitor<'a> for ModuleWalker<'a> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        match statement {
            Stmt::FunctionDef(definition) => self.bind(&definition.name),
            Stmt::ClassDef(class) => self.bind(&class.name),
            Stmt::Global(global) => global.names.iter().for_each(|name| self.bind(name)),
            Stmt::Nonlocal(nonlocal) => nonlocal.names.iter().for_each(|name| self.bind(name)),
            _ => self.record_exports(statement),
        }
        walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'a Expr) {
        match expression {
            Expr::Name(name) => match name.ctx {
                ExprContext::Load => self.read(&name.id),
                ExprContext::Store | ExprContext::Del => self.bind(&name.id),
                ExprContext::Invalid => {}
            },
            // `_X[k] = v`, `del _X[k]`, and `_X.attr = v` mutate the binding in place, so it is
            // state shared across calls rather than a value the consumer merely reads.
            Expr::Subscript(subscript) if !subscript.ctx.is_load() => {
                self.record_mutation(&subscript.value);
            }
            Expr::Attribute(attribute) if !attribute.ctx.is_load() => {
                self.record_mutation(&attribute.value);
            }
            _ => {}
        }
        walk_expr(self, expression);
    }

    fn visit_parameter(&mut self, parameter: &'a Parameter) {
        self.bind(&parameter.name);
        walk_parameter(self, parameter);
    }

    fn visit_alias(&mut self, alias: &'a Alias) {
        let bound = alias.asname.as_ref().unwrap_or(&alias.name);
        // `import a.b` binds `a`; the dotted tail is attribute access on it.
        self.bind(bound.split('.').next().unwrap_or(bound));
    }

    fn visit_except_handler(&mut self, handler: &'a ExceptHandler) {
        let ExceptHandler::ExceptHandler(clause) = handler;
        if let Some(name) = &clause.name {
            self.bind(name);
        }
        walk_except_handler(self, handler);
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        let bound = match pattern {
            Pattern::MatchAs(pattern) => pattern.name.as_ref(),
            Pattern::MatchStar(pattern) => pattern.name.as_ref(),
            Pattern::MatchMapping(pattern) => pattern.rest.as_ref(),
            _ => None,
        };
        if let Some(name) = bound {
            self.bind(name);
        }
        walk_pattern(self, pattern);
    }

    fn visit_type_param(&mut self, type_param: &'a TypeParam) {
        let name = match type_param {
            TypeParam::TypeVar(parameter) => &parameter.name,
            TypeParam::TypeVarTuple(parameter) => &parameter.name,
            TypeParam::ParamSpec(parameter) => &parameter.name,
        };
        self.bind(name);
        walk_type_param(self, type_param);
    }
}
