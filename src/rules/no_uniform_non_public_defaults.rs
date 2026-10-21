use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use ruff_python_ast::Alias;
use ruff_python_ast::ExceptHandler;
use ruff_python_ast::Expr;
use ruff_python_ast::ExprCall;
use ruff_python_ast::ExprContext;
use ruff_python_ast::InterpolatedStringElement;
use ruff_python_ast::Parameter;
use ruff_python_ast::Pattern;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtFunctionDef;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_except_handler;
use ruff_python_ast::visitor::walk_expr;
use ruff_python_ast::visitor::walk_interpolated_string_element;
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
    // A wildcard import may rebind any name, so no call can be tied to a definition, and a
    // rebound `staticmethod` or `classmethod` hides how a method binds its receiver.
    if ["*", "staticmethod", "classmethod"]
        .iter()
        .any(|name| bindings.counts.contains_key(name))
    {
        return Vec::new();
    }
    let candidates = find_candidates(path, statements, &bindings.counts);
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut walker = CallWalker {
        usages: candidates
            .iter()
            .map(|candidate| {
                let owner = bindings
                    .method_owners
                    .get(candidate.definition.name.as_str())
                    .copied();
                (
                    candidate.key(),
                    Usage {
                        // A class name bound twice, such as by a parameter, may name another
                        // object, so calls through it cannot be tied to the method.
                        is_opaque: owner.is_some_and(|owner| bindings.counts[owner] != 1),
                        // Only an instance method's receiver comes from the instance, so only its
                        // call through the class passes the receiver as the first argument.
                        takes_instance: candidate.is_method
                            && candidate.definition.decorator_list.is_empty(),
                        owner,
                        is_owner_replaceable: owner
                            .is_some_and(|owner| bindings.replaceable_classes.contains(owner)),
                        ..Usage::default()
                    },
                )
            })
            .collect(),
        scopes: Vec::new(),
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
    is_owner_replaceable: bool,
}

struct PassedInputs {
    positional_count: usize,
    keywords: HashSet<String>,
}

struct CallWalker<'a> {
    usages: HashMap<(&'a str, bool), Usage<'a>>,
    // The enclosing classes and functions, innermost last.
    scopes: Vec<Scope<'a>>,
}

enum Scope<'a> {
    Class(&'a str),
    // The receiver name, `self` or `cls`, that a method directly inside a class binds to its
    // instance or class and never rebinds.
    Function(Option<&'a str>),
}

// `self` names the instance only in an undecorated method and `cls` the class only in a
// classmethod, and only while the method's first input keeps that name and its body never rebinds
// it. A static method, a renamed receiver, or `self = other` leaves the name unresolved.
fn trusted_receiver(function: &StmtFunctionDef) -> Option<&'static str> {
    let receiver = match function.decorator_list.as_slice() {
        [] => "self",
        [decorator] if matches!(&decorator.expression, Expr::Name(name) if name.id == "classmethod") => {
            "cls"
        }
        _ => return None,
    };
    let parameters = &function.parameters;
    let first = parameters
        .posonlyargs
        .iter()
        .chain(&parameters.args)
        .next()?;
    if first.name().as_str() != receiver {
        return None;
    }
    let mut bindings = BindingCollector::default();
    bindings.visit_body(&function.body);
    (!bindings.counts.contains_key(receiver)).then_some(receiver)
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
        // The class name in `_Class__name` may itself contain `__`, so every split is tried.
        let mangled = attribute_name
            .strip_prefix('_')
            .into_iter()
            .flat_map(|rest| {
                rest.match_indices("__")
                    .map(move |(index, _)| (&rest[index..], true))
            });
        for key in std::iter::once((attribute_name, false)).chain(mangled) {
            if let Some(usage) = self.usages.get_mut(&key) {
                usage.is_opaque = true;
            }
        }
    }

    // Inside its class body a method is a plain name, such as in `__call__ = _load`, which the walk
    // does not tie to an instance, so such a reference makes the method's calls untrusted.
    fn mark_bare_method_reference(&mut self, name: &'a str) {
        if let Some(usage) = self.usages.get_mut(&(name, true)) {
            usage.is_opaque = true;
        }
    }

    fn mark_string_reference(&mut self, value: &'a str) {
        self.mark_unresolved_attribute(value);
        self.mark_bare_method_reference(value);
    }

    fn record_call(&mut self, call: &'a ExprCall) {
        match &*call.func {
            Expr::Attribute(attribute) => self.mark_unresolved_attribute(attribute.attr.as_str()),
            Expr::Name(name) => self.mark_bare_method_reference(name.id.as_str()),
            _ => {}
        }
        // A method call is tied to the candidate only through `self` or `cls` inside the class that
        // defines it, or through that class's name; any other receiver may hold an unrelated object
        // with a method of the same name. Through `cls` or the class, an instance method takes the
        // receiver as its first argument, which shifts every positional input by one.
        // A nested function or lambda may bind `self` and `cls` to anything, so only the innermost
        // method's own receiver counts.
        let method_receiver = match self.scopes.as_slice() {
            [.., Scope::Class(class), Scope::Function(Some(receiver))] => Some((*class, *receiver)),
            _ => None,
        };
        let receiver = match &*call.func {
            Expr::Attribute(attribute) => Some(match &*attribute.value {
                Expr::Name(name) => match method_receiver {
                    Some((class, receiver)) if name.id == receiver => {
                        (Some(class), receiver == "cls")
                    }
                    _ => (Some(name.id.as_str()), true),
                },
                _ => (None, false),
            }),
            _ => None,
        };
        let Some(usage) = self.usage_for(&call.func) else {
            return;
        };
        let is_unbound = receiver.is_some_and(|(class, is_through_class)| {
            class.is_none()
                || class != usage.owner
                || (is_through_class && (usage.takes_instance || usage.is_owner_replaceable))
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
        // Decorators, bases, defaults, and annotations run in the enclosing scope, so only the body
        // enters the new one.
        let (scope, body) = match statement {
            Stmt::ClassDef(class) => {
                for decorator in &class.decorator_list {
                    self.visit_decorator(decorator);
                }
                if let Some(type_params) = &class.type_params {
                    self.visit_type_params(type_params);
                }
                if let Some(arguments) = &class.arguments {
                    self.visit_arguments(arguments);
                }
                (Scope::Class(class.name.as_str()), &class.body)
            }
            Stmt::FunctionDef(function) => {
                for decorator in &function.decorator_list {
                    self.visit_decorator(decorator);
                }
                if let Some(type_params) = &function.type_params {
                    self.visit_type_params(type_params);
                }
                self.visit_parameters(&function.parameters);
                if let Some(returns) = &function.returns {
                    self.visit_annotation(returns);
                }
                let receiver = matches!(self.scopes.last(), Some(Scope::Class(_)))
                    .then(|| trusted_receiver(function))
                    .flatten();
                (Scope::Function(receiver), &function.body)
            }
            _ => return walk_stmt(self, statement),
        };
        self.scopes.push(scope);
        self.visit_body(body);
        self.scopes.pop();
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
                match expression {
                    Expr::Attribute(attribute) => {
                        self.mark_unresolved_attribute(attribute.attr.as_str());
                    }
                    Expr::Name(name) => self.mark_bare_method_reference(name.id.as_str()),
                    _ => {}
                }
                if let Some(usage) = self.usage_for(expression) {
                    usage.is_opaque = true;
                }
                walk_expr(self, expression);
            }
            // A string naming a candidate may reach it through `getattr` or a namespace lookup.
            Expr::StringLiteral(string) => {
                self.mark_string_reference(string.value.to_str());
            }
            Expr::Lambda(lambda) => {
                if let Some(parameters) = &lambda.parameters {
                    self.visit_parameters(parameters);
                }
                self.scopes.push(Scope::Function(None));
                self.visit_expr(&lambda.body);
                self.scopes.pop();
            }
            _ => walk_expr(self, expression),
        }
    }

    fn visit_interpolated_string_element(&mut self, element: &'a InterpolatedStringElement) {
        if let InterpolatedStringElement::Literal(literal) = element {
            self.mark_string_reference(&literal.value);
        }
        walk_interpolated_string_element(self, element);
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
        // Python mangles a method's `__name` input to `_Class__name`, which callers may spell
        // either way, so such an input is left alone.
        .filter(|(_, input)| {
            let is_mangled =
                candidate.is_method && input.name.starts_with("__") && !input.name.ends_with("__");
            !input.is_required && !is_mangled
        })
        .filter_map(|(index, input)| {
            let passed_count = usage
                .calls
                .iter()
                .filter(|call| {
                    let accepts_keyword =
                        input.is_positional_or_keyword || index >= positional_count;
                    (index < positional_count && index < call.positional_count)
                        || (accepts_keyword && call.keywords.contains(input.name))
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
// calls are walked. A nested definition, parameter, import, assignment, pattern capture, or
// exception name that reuses a candidate's name may take its calls, so those bindings are counted
// too.
#[derive(Default)]
struct BindingCollector<'a> {
    method_owners: HashMap<&'a str, &'a str>,
    // A class decorator or a metaclass, given directly or through a base, may return another
    // object, so such a class's name may not name the class.
    replaceable_classes: HashSet<&'a str>,
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
                if !class.decorator_list.is_empty() || class.arguments.is_some() {
                    self.replaceable_classes.insert(class.name.as_str());
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
        // `import a.b` binds `a`.
        let name = match &alias.asname {
            Some(asname) => asname.as_str(),
            None => alias.name.split('.').next().unwrap_or_default(),
        };
        self.bind(name);
    }
}
