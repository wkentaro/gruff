use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use ruff_python_ast::Alias;
use ruff_python_ast::ExceptHandler;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprCall;
use ruff_python_ast::ExprContext;
use ruff_python_ast::Parameter;
use ruff_python_ast::Pattern;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtFunctionDef;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_except_handler;
use ruff_python_ast::visitor::walk_expr;
use ruff_python_ast::visitor::walk_parameter;
use ruff_python_ast::visitor::walk_pattern;
use ruff_python_ast::visitor::walk_stmt;

use super::Diagnostic;
use crate::analysis::classify_inputs;
use crate::analysis::find_non_public_definitions;
use crate::analysis::is_non_public_name;

pub(crate) const CODE: &str = "GR002";
pub(crate) const NAME: &str = "no-uniform-non-public-defaults";
pub(crate) const SUMMARY: &str =
    "Some calls in its module pass a module-private default and others omit it.";

pub(crate) fn check(path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    let mut bindings = BindingCollector::default();
    bindings.visit_body(statements);
    let candidates = find_candidates(path, statements, &bindings.counts);
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut walker = CallWalker {
        usages: candidates
            .iter()
            .map(|candidate| {
                (
                    candidate.key(),
                    Usage {
                        // Only an instance method's receiver comes from the instance, so only its
                        // call through the class passes the receiver as the first argument.
                        takes_instance: candidate.is_method
                            && candidate.definition.decorator_list.is_empty(),
                        owner: bindings
                            .method_owners
                            .get(candidate.definition.name.as_str())
                            .copied(),
                        ..Usage::default()
                    },
                )
            })
            .collect(),
        class_stack: Vec::new(),
    };
    walker.visit_body(statements);

    candidates
        .iter()
        .flat_map(|candidate| {
            uniform_default_diagnostics(candidate, &walker.usages[&candidate.key()])
        })
        .collect()
}

struct Candidate<'a> {
    definition: &'a StmtFunctionDef,
    is_method: bool,
}

impl<'a> Candidate<'a> {
    // A bare name can only call a function, and an attribute can only call a method.
    fn key(&self) -> (&'a str, bool) {
        (self.definition.name.as_str(), self.is_method)
    }
}

// Only a definition whose own name is non-public has every caller in its module, so only its
// calls can show how callers use a default. A definition whose name something else in the module
// also binds, or that a decorator other than `staticmethod` or `classmethod` may rewrap, has calls
// the walk cannot attribute, so it is left alone.
fn find_candidates<'a>(
    path: &Path,
    statements: &'a [Stmt],
    binding_counts: &HashMap<&str, usize>,
) -> Vec<Candidate<'a>> {
    find_non_public_definitions(path, statements)
        .into_iter()
        .filter(|(definition, _)| {
            is_non_public_name(&definition.name)
                && binding_counts.get(definition.name.as_str()) == Some(&1)
                && definition.decorator_list.iter().all(|decorator| {
                    matches!(
                        &decorator.expression,
                        Expr::Name(name) if name.id == "staticmethod" || name.id == "classmethod"
                    )
                })
        })
        .map(|(definition, is_method)| Candidate {
            definition,
            is_method,
        })
        .collect()
}

#[derive(Default)]
struct Usage<'a> {
    calls: Vec<PassedInputs>,
    // A reference that is not a plain call, such as a callback, a `functools.partial` argument, or
    // a call unpacking `*args` or `**kwargs`, may pass any input, so no call count can be trusted.
    is_opaque: bool,
    takes_instance: bool,
    owner: Option<&'a str>,
}

struct PassedInputs {
    positional_count: usize,
    keywords: HashSet<String>,
}

struct CallWalker<'a> {
    usages: HashMap<(&'a str, bool), Usage<'a>>,
    class_stack: Vec<&'a str>,
}

impl<'a> CallWalker<'a> {
    fn usage_for(&mut self, reference: &'a Expr) -> Option<&mut Usage<'a>> {
        match reference {
            Expr::Name(name) => self.usages.get_mut(&(name.id.as_str(), false)),
            Expr::Attribute(attribute) => self.usages.get_mut(&(attribute.attr.as_str(), true)),
            _ => None,
        }
    }

    // An attribute named like a function may reach it through its module object, and a
    // double-leading method name may be spelled `_Class__name` from outside its class. The walk
    // cannot resolve either, so such a reference makes the definition's calls untrusted.
    fn mark_unresolved_attribute(&mut self, attribute_name: &'a str) {
        let mangled = attribute_name
            .strip_prefix('_')
            .and_then(|rest| rest.find("__").map(|index| (&rest[index..], true)));
        for key in [Some((attribute_name, false)), mangled]
            .into_iter()
            .flatten()
        {
            if let Some(usage) = self.usages.get_mut(&key) {
                usage.is_opaque = true;
            }
        }
    }

    fn record_call(&mut self, call: &'a ExprCall) {
        if let Expr::Attribute(attribute) = &*call.func {
            self.mark_unresolved_attribute(attribute.attr.as_str());
        }
        // A method call is tied to the candidate only through `self` or `cls` inside the class that
        // defines it, or through that class's name; any other receiver may hold an unrelated object
        // with a method of the same name. Through `cls` or the class, an instance method takes the
        // receiver as its first argument, which shifts every positional input by one.
        let receiver = match &*call.func {
            Expr::Attribute(attribute) => Some(match &*attribute.value {
                Expr::Name(name) if name.id == "self" => (self.class_stack.last().copied(), false),
                Expr::Name(name) if name.id == "cls" => (self.class_stack.last().copied(), true),
                Expr::Name(name) => (Some(name.id.as_str()), true),
                _ => (None, false),
            }),
            _ => None,
        };
        let Some(usage) = self.usage_for(&call.func) else {
            return;
        };
        let is_unbound = receiver.is_some_and(|(class, is_through_class)| {
            class.is_none() || class != usage.owner || (is_through_class && usage.takes_instance)
        });
        let is_unpacked = call
            .arguments
            .args
            .iter()
            .any(|argument| matches!(argument, Expr::Starred(_)))
            || call
                .arguments
                .keywords
                .iter()
                .any(|keyword| keyword.arg.is_none());
        if is_unbound || is_unpacked {
            usage.is_opaque = true;
        } else {
            usage.calls.push(PassedInputs {
                positional_count: call.arguments.args.len(),
                keywords: call
                    .arguments
                    .keywords
                    .iter()
                    .filter_map(|keyword| keyword.arg.as_ref().map(ToString::to_string))
                    .collect(),
            });
        }
    }
}

impl<'a> Visitor<'a> for CallWalker<'a> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        if let Stmt::ClassDef(class) = statement {
            self.class_stack.push(class.name.as_str());
            walk_stmt(self, statement);
            self.class_stack.pop();
        } else {
            walk_stmt(self, statement);
        }
    }

    fn visit_expr(&mut self, expression: &'a Expr) {
        match expression {
            Expr::Call(call) => {
                self.record_call(call);
                // The callee is the call itself rather than an escaping reference, so only an
                // attribute callee's receiver and the arguments are walked.
                match &*call.func {
                    Expr::Name(_) => {}
                    Expr::Attribute(attribute) => self.visit_expr(&attribute.value),
                    callee => self.visit_expr(callee),
                }
                self.visit_arguments(&call.arguments);
            }
            Expr::Name(_) | Expr::Attribute(_) => {
                if let Expr::Attribute(attribute) = expression {
                    self.mark_unresolved_attribute(attribute.attr.as_str());
                }
                if let Some(usage) = self.usage_for(expression) {
                    usage.is_opaque = true;
                }
                walk_expr(self, expression);
            }
            _ => walk_expr(self, expression),
        }
    }
}

fn uniform_default_diagnostics(candidate: &Candidate, usage: &Usage<'_>) -> Vec<Diagnostic> {
    if usage.is_opaque || usage.calls.is_empty() {
        return Vec::new();
    }
    let inputs = classify_inputs(candidate.definition, candidate.is_method);
    // Inputs list positional ones, receiver excluded, before keyword-only ones.
    let positional_count = inputs.len() - candidate.definition.parameters.kwonlyargs.len();
    inputs
        .iter()
        .enumerate()
        .filter(|(_, input)| !input.is_required)
        .filter_map(|(index, input)| {
            let passed_count = usage
                .calls
                .iter()
                .filter(|call| {
                    (index < positional_count && index < call.positional_count)
                        || call.keywords.contains(input.name)
                })
                .count();
            let message = if passed_count == 0 {
                format!(
                    "No call passes `{}`; remove the input and use its default inside `{}`",
                    input.name, candidate.definition.name
                )
            } else if passed_count == usage.calls.len() {
                format!("Every call passes `{}`; remove its default", input.name)
            } else {
                return None;
            };
            Some(Diagnostic {
                message,
                range: input.range,
                noqa_offset: None,
            })
        })
        .collect()
}

// A call above a class may still name its methods, so each method's class is collected before the
// calls are walked. A nested definition, parameter, import, assignment, pattern capture, or exception name that reuses a candidate's name may take its
// calls, so those bindings are counted too.
#[derive(Default)]
struct BindingCollector<'a> {
    method_owners: HashMap<&'a str, &'a str>,
    counts: HashMap<&'a str, usize>,
}

impl<'a> BindingCollector<'a> {
    fn bind(&mut self, name: &'a str) {
        *self.counts.entry(name).or_default() += 1;
    }
}

impl<'a> Visitor<'a> for BindingCollector<'a> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        match statement {
            Stmt::ClassDef(class) => {
                for method in &class.body {
                    if let Stmt::FunctionDef(method) = method {
                        self.method_owners
                            .insert(method.name.as_str(), class.name.as_str());
                    }
                }
                self.bind(class.name.as_str());
            }
            Stmt::FunctionDef(function) => self.bind(function.name.as_str()),
            _ => {}
        }
        walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'a Expr) {
        if let Expr::Name(name) = expression
            && name.ctx == ExprContext::Store
        {
            self.bind(name.id.as_str());
        }
        walk_expr(self, expression);
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
        let ExceptHandler::ExceptHandler(handler_body) = handler;
        if let Some(name) = &handler_body.name {
            self.bind(name.as_str());
        }
        walk_except_handler(self, handler);
    }

    fn visit_parameter(&mut self, parameter: &'a Parameter) {
        self.bind(parameter.name.as_str());
        walk_parameter(self, parameter);
    }

    fn visit_alias(&mut self, alias: &'a Alias) {
        self.bind(alias.asname.as_ref().unwrap_or(&alias.name).as_str());
    }
}
