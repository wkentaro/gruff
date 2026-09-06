use std::collections::HashSet;
use std::path::Path;

use ruff_python_ast::Expr;
use ruff_python_ast::ExprContext;
use ruff_python_ast::Stmt;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_expr;
use ruff_python_ast::visitor::walk_stmt;
use ruff_text_size::Ranged;

use super::Diagnostic;

pub(crate) const CODE: &str = "GR012";
pub(crate) const NAME: &str = "public-data-properties";
pub(crate) const SUMMARY: &str =
    "Public instance data uses explicit properties with underscore-prefixed storage.";

pub(crate) fn check(_path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    let mut visitor = ClassVisitor {
        diagnostics: Vec::new(),
    };
    visitor.visit_body(statements);
    visitor.diagnostics
}

struct ClassVisitor {
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Visitor<'a> for ClassVisitor {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        if let Stmt::ClassDef(class) = statement {
            let methods: Vec<_> = class
                .body
                .iter()
                .filter_map(Stmt::as_function_def_stmt)
                .collect();
            let mut properties = HashSet::new();
            let mut setters = HashSet::new();
            for method in &methods {
                for decorator in &method.decorator_list {
                    if matches_builtin(&decorator.expression, "property") {
                        properties.insert(method.name.as_str());
                    }
                    if let Expr::Attribute(attribute) = &decorator.expression
                        && attribute.attr.as_str() == "setter"
                        && match attribute.value.as_ref() {
                            Expr::Name(name) => name.id == method.name.as_str(),
                            Expr::Attribute(base) => base.attr.as_str() == method.name.as_str(),
                            _ => false,
                        }
                    {
                        setters.insert(method.name.as_str());
                        properties.insert(method.name.as_str());
                    }
                }
            }
            for method in methods {
                // These hooks receive a class even without a classmethod decorator.
                if matches!(
                    method.name.as_str(),
                    "__new__" | "__init_subclass__" | "__class_getitem__"
                ) || method.decorator_list.iter().any(|decorator| {
                    matches_builtin(&decorator.expression, "staticmethod")
                        || matches_builtin(&decorator.expression, "classmethod")
                }) {
                    continue;
                }
                let Some(receiver) = method
                    .parameters
                    .posonlyargs
                    .first()
                    .or_else(|| method.parameters.args.first())
                else {
                    continue;
                };
                AttributeVisitor {
                    receiver: receiver.name().as_str(),
                    properties: &properties,
                    setters: &setters,
                    is_annotation_only: false,
                    diagnostics: &mut self.diagnostics,
                }
                .visit_body(&method.body);
            }
        }
        // Each nested class owns its own receiver and property declarations.
        walk_stmt(self, statement);
    }
}

fn matches_builtin(expression: &Expr, expected: &str) -> bool {
    match expression {
        Expr::Name(name) => name.id == expected,
        Expr::Attribute(attribute) => {
            attribute.attr.as_str() == expected
                && matches!(attribute.value.as_ref(), Expr::Name(name) if name.id == "builtins")
        }
        _ => false,
    }
}

struct AttributeVisitor<'a, 'b> {
    receiver: &'a str,
    properties: &'b HashSet<&'a str>,
    setters: &'b HashSet<&'a str>,
    is_annotation_only: bool,
    diagnostics: &'b mut Vec<Diagnostic>,
}

impl<'a> Visitor<'a> for AttributeVisitor<'a, '_> {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        match statement {
            // Nested scopes can shadow the receiver; no binding or alias inference is attempted.
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            Stmt::AnnAssign(assignment) if assignment.value.is_none() => {
                self.is_annotation_only = true;
                self.visit_expr(&assignment.target);
                self.is_annotation_only = false;
            }
            _ => walk_stmt(self, statement),
        }
    }

    fn visit_expr(&mut self, expression: &'a Expr) {
        if matches!(expression, Expr::Lambda(_)) {
            return;
        }
        if let Expr::Attribute(attribute) = expression
            && attribute.ctx == ExprContext::Store
            && !attribute.attr.starts_with('_')
            && matches!(attribute.value.as_ref(), Expr::Name(name) if name.id == self.receiver)
            && !self.setters.contains(attribute.attr.as_str())
            && !(self.is_annotation_only && self.properties.contains(attribute.attr.as_str()))
        {
            self.diagnostics.push(Diagnostic {
                message: format!(
                    "Public instance attribute `{}` requires a property setter for writes; use underscore-prefixed storage for internal state",
                    attribute.attr
                ),
                range: attribute.attr.range(),
                noqa_offset: None,
            });
        }
        walk_expr(self, expression);
    }
}
