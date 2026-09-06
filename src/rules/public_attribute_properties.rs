use ruff_python_ast::Expr;
use ruff_python_ast::ExprContext;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtClassDef;
use ruff_python_ast::StmtFunctionDef;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_expr;
use ruff_python_ast::visitor::walk_stmt;
use ruff_text_size::Ranged;

use super::Diagnostic;
pub(crate) const CODE: &str = "GR012";
pub(crate) const NAME: &str = "public-attribute-properties";
pub(crate) const SUMMARY: &str =
    "Public instance state uses underscore-prefixed storage and an explicit property.";

pub(crate) fn check(_path: &std::path::Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    let mut checker = Checker {
        diagnostics: Vec::new(),
    };
    checker.visit_module(statements);
    checker.diagnostics
}

struct Checker {
    diagnostics: Vec<Diagnostic>,
}

impl Checker {
    fn visit_module(&mut self, statements: &[Stmt]) {
        for statement in statements {
            match statement {
                Stmt::ClassDef(class) => self.visit_class(class),
                Stmt::FunctionDef(function) => self.visit_function(function, Receiver::None),
                _ => self.visit_external_statement(statement),
            }
        }
    }

    fn visit_class(&mut self, class: &StmtClassDef) {
        for statement in &class.body {
            match statement {
                Stmt::ClassDef(nested) => self.visit_class(nested),
                Stmt::FunctionDef(function) => {
                    self.visit_function(function, method_receiver(function))
                }
                _ => self.visit_external_statement(statement),
            }
        }
    }

    fn visit_function(&mut self, function: &StmtFunctionDef, receiver: Receiver) {
        let property_setter = is_property_setter(function);
        let mut visitor = AttributeVisitor {
            receiver,
            property_setter,
            diagnostics: Vec::new(),
        };
        visitor.visit_body(&function.body);
        self.diagnostics.extend(visitor.diagnostics);
    }

    fn visit_external_statement(&mut self, statement: &Stmt) {
        let mut visitor = AttributeVisitor {
            receiver: Receiver::None,
            property_setter: false,
            diagnostics: Vec::new(),
        };
        visitor.visit_stmt(statement);
        self.diagnostics.extend(visitor.diagnostics);
    }
}

#[derive(Clone, Debug)]
enum Receiver {
    Instance(String),
    Class(String),
    None,
}

fn method_receiver(function: &StmtFunctionDef) -> Receiver {
    let receiver = function
        .parameters
        .posonlyargs
        .first()
        .or_else(|| function.parameters.args.first())
        .map(|parameter| parameter.name().to_string());

    if function.decorator_list.iter().any(
        |decorator| matches!(&decorator.expression, Expr::Name(name) if name.id == "staticmethod"),
    ) {
        Receiver::None
    } else if function.decorator_list.iter().any(
        |decorator| matches!(&decorator.expression, Expr::Name(name) if name.id == "classmethod"),
    ) {
        receiver.map_or(Receiver::None, Receiver::Class)
    } else {
        receiver.map_or(Receiver::None, Receiver::Instance)
    }
}

fn is_property_setter(function: &StmtFunctionDef) -> bool {
    function.decorator_list.iter().any(|decorator| {
        matches!(&decorator.expression, Expr::Attribute(attribute) if attribute.attr.as_str() == "setter")
    })
}

fn is_public_attribute(name: &str) -> bool {
    !name.starts_with('_')
}

fn receiver_name(receiver: &Receiver) -> Option<&str> {
    match receiver {
        Receiver::Instance(name) | Receiver::Class(name) => Some(name),
        Receiver::None => None,
    }
}

fn is_receiver(expression: &Expr, receiver: &Receiver) -> bool {
    let Some(expected) = receiver_name(receiver) else {
        return false;
    };
    matches!(expression, Expr::Name(name) if name.id == expected)
}

struct AttributeVisitor {
    receiver: Receiver,
    property_setter: bool,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Visitor<'a> for AttributeVisitor {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        // A nested definition has a new receiver and is visited separately by Checker when it is
        // a class member. Do not interpret its `self` spelling using the enclosing method.
        if matches!(statement, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
            return;
        }
        walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'a Expr) {
        if let Expr::Attribute(attribute) = expression {
            match attribute.ctx {
                ExprContext::Store
                    if matches!(self.receiver, Receiver::Instance(_))
                        && !self.property_setter
                        && is_public_attribute(attribute.attr.as_str())
                        && is_receiver(&attribute.value, &self.receiver) =>
                {
                    self.diagnostics.push(Diagnostic {
                        message: format!(
                            "Public instance attribute `{}` must use underscore-prefixed storage and an explicit property",
                            attribute.attr
                        ),
                        range: attribute.range(),
                        noqa_offset: Some(attribute.range().start()),
                    });
                }
                _ => {}
            }
        }
        walk_expr(self, expression);
    }
}
