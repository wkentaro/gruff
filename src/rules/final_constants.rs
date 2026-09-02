use std::path::Path;

use ruff_python_ast::Expr;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtClassDef;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_stmt;

use super::Diagnostic;

pub(crate) const CODE: &str = "GR004";
pub(crate) const NAME: &str = "final-constants";
pub(crate) const SUMMARY: &str = "Uppercase names and `Final` annotations appear together.";

pub(crate) fn check(_path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    let mut visitor = FinalConstantVisitor {
        is_enum_body: false,
        diagnostics: Vec::new(),
    };
    visitor.visit_body(statements);
    visitor.diagnostics
}

fn is_uppercase(name: &str) -> bool {
    let mut characters = name.trim_start_matches('_').bytes();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_uppercase())
        && characters.all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == b'_'
        })
}

fn is_annotation_named(annotation: &Expr, expected: &str) -> bool {
    match annotation {
        Expr::Name(name) => name.id == expected,
        Expr::Attribute(attribute) => attribute.attr.as_str() == expected,
        Expr::Subscript(subscript) => is_annotation_named(&subscript.value, expected),
        _ => false,
    }
}

fn is_enum_class(class: &StmtClassDef) -> bool {
    const ENUM_BASES: &[&str] = &["Enum", "IntEnum", "StrEnum", "ReprEnum", "Flag", "IntFlag"];

    class.arguments.as_ref().is_some_and(|arguments| {
        arguments.args.iter().any(|base| match base {
            Expr::Name(name) => ENUM_BASES.contains(&name.id.as_str()),
            Expr::Attribute(attribute) => ENUM_BASES.contains(&attribute.attr.as_str()),
            _ => false,
        })
    })
}

struct FinalConstantVisitor {
    is_enum_body: bool,
    diagnostics: Vec<Diagnostic>,
}

impl FinalConstantVisitor {
    fn check_binding(&mut self, name: &ruff_python_ast::ExprName, annotation: Option<&Expr>) {
        if annotation.is_some_and(|annotation| is_annotation_named(annotation, "TypeAlias")) {
            return;
        }

        let is_uppercase = is_uppercase(name.id.as_str());
        let is_final =
            annotation.is_some_and(|annotation| is_annotation_named(annotation, "Final"));
        if is_uppercase && !is_final {
            self.diagnostics.push(Diagnostic {
                message: format!("Constant {} must be annotated Final", name.id),
                range: name.range,
                noqa_offset: Some(name.range.start()),
            });
        } else if is_final && !is_uppercase {
            self.diagnostics.push(Diagnostic {
                message: format!(
                    "Final binding {} must be named in UPPER_SNAKE_CASE",
                    name.id
                ),
                range: name.range,
                noqa_offset: Some(name.range.start()),
            });
        }
    }
}

impl<'a> Visitor<'a> for FinalConstantVisitor {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        let previous_is_enum_body = self.is_enum_body;
        match statement {
            Stmt::Assign(assignment) if !self.is_enum_body => {
                if let [Expr::Name(name)] = assignment.targets.as_slice() {
                    self.check_binding(name, None);
                }
            }
            Stmt::AnnAssign(assignment) if !self.is_enum_body => {
                if let Expr::Name(name) = assignment.target.as_ref() {
                    self.check_binding(name, Some(&assignment.annotation));
                }
            }
            Stmt::FunctionDef(_) => self.is_enum_body = false,
            Stmt::ClassDef(class) => self.is_enum_body = is_enum_class(class),
            _ => {}
        }
        walk_stmt(self, statement);
        self.is_enum_body = previous_is_enum_body;
    }
}
