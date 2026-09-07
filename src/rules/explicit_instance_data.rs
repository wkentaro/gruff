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
pub(crate) const NAME: &str = "explicit-instance-data";
pub(crate) const SUMMARY: &str =
    "Public instance data is explicit through class-body annotations or properties.";

pub(crate) fn check(_path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    let mut non_instance_aliases = HashSet::new();
    collect_non_instance_aliases(statements, &mut non_instance_aliases);
    let mut visitor = ClassVisitor {
        diagnostics: Vec::new(),
        non_instance_aliases,
    };
    visitor.visit_body(statements);
    visitor.diagnostics
}

struct ClassVisitor {
    diagnostics: Vec<Diagnostic>,
    non_instance_aliases: HashSet<String>,
}

impl<'a> Visitor<'a> for ClassVisitor {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        let outer_aliases = if let Stmt::FunctionDef(function) = statement {
            let outer = self.non_instance_aliases.clone();
            collect_non_instance_aliases(&function.body, &mut self.non_instance_aliases);
            Some(outer)
        } else {
            None
        };
        if let Stmt::ClassDef(class) = statement {
            // Class namespaces are not enclosing lexical scopes for methods or nested classes.
            let mut class_aliases = self.non_instance_aliases.clone();
            collect_non_instance_aliases(&class.body, &mut class_aliases);
            let methods: Vec<_> = class
                .body
                .iter()
                .filter_map(Stmt::as_function_def_stmt)
                .collect();
            let declared_fields: HashSet<_> = class
                .body
                .iter()
                .filter_map(|statement| {
                    if let Stmt::AnnAssign(assignment) = statement
                        && let Expr::Name(name) = assignment.target.as_ref()
                        && !is_non_instance_annotation(&assignment.annotation, &class_aliases)
                    {
                        Some(name.id.as_str())
                    } else {
                        None
                    }
                })
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
                    declared_fields: &declared_fields,
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
        if let Some(outer) = outer_aliases {
            self.non_instance_aliases = outer;
        }
    }
}

fn collect_non_instance_aliases(statements: &[Stmt], aliases: &mut HashSet<String>) {
    struct ImportVisitor<'a>(&'a mut HashSet<String>);

    impl<'a> Visitor<'a> for ImportVisitor<'_> {
        fn visit_stmt(&mut self, statement: &'a Stmt) {
            match statement {
                Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
                Stmt::ImportFrom(import) if import.level == 0 => {
                    for alias in &import.names {
                        if matches!(
                            (
                                import.module.as_ref().map(|name| name.as_str()),
                                alias.name.as_str()
                            ),
                            (Some("typing" | "typing_extensions"), "ClassVar")
                                | (Some("dataclasses"), "InitVar")
                        ) {
                            self.0
                                .insert(alias.asname.as_ref().unwrap_or(&alias.name).to_string());
                        }
                    }
                }
                _ => walk_stmt(self, statement),
            }
        }
    }

    ImportVisitor(aliases).visit_body(statements);
}

fn is_non_instance_annotation(annotation: &Expr, aliases: &HashSet<String>) -> bool {
    match annotation {
        Expr::Subscript(subscript) => is_non_instance_annotation(&subscript.value, aliases),
        Expr::StringLiteral(literal) => {
            ruff_python_parser::parse_expression(literal.value.to_str())
                .is_ok_and(|parsed| is_non_instance_annotation(parsed.expr(), aliases))
        }
        Expr::Name(name) => {
            matches!(name.id.as_str(), "ClassVar" | "InitVar") || aliases.contains(name.id.as_str())
        }
        Expr::Attribute(attribute) => matches!(attribute.attr.as_str(), "ClassVar" | "InitVar"),
        _ => false,
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
    declared_fields: &'b HashSet<&'a str>,
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
            && (!self.declared_fields.contains(attribute.attr.as_str())
                || self.properties.contains(attribute.attr.as_str()))
            && !(self.is_annotation_only && self.properties.contains(attribute.attr.as_str()))
        {
            self.diagnostics.push(Diagnostic {
                message: if self.properties.contains(attribute.attr.as_str()) {
                    format!(
                        "Write to getter-only property `{}`; use private storage, or declare a setter for intentional public writes.",
                        attribute.attr
                    )
                } else {
                    format!(
                        "Instance data field `{}` is implicit; use private storage for internal state, or declare instance data or expose a property for public access.",
                        attribute.attr
                    )
                },
                range: attribute.attr.range(),
                noqa_offset: None,
            });
        }
        walk_expr(self, expression);
    }
}
