use std::path::Path;

use ruff_python_ast::ExceptHandler;
use ruff_python_ast::Expr;
use ruff_python_ast::Stmt;
use ruff_python_ast::helpers::is_docstring_stmt;
use ruff_python_ast::visitor::Visitor;
use ruff_python_ast::visitor::walk_stmt;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;
use ruff_text_size::TextSize;

use super::Diagnostic;

pub(crate) const CODE: &str = "GR008";
pub(crate) const NAME: &str = "no-exception-swallowing-tests";
pub(crate) const SUMMARY: &str = "Tests let exceptions propagate instead of swallowing them.";

pub(crate) fn check(path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_test_file_name)
    {
        return Vec::new();
    }

    let mut visitor = SwallowingHandlerVisitor {
        is_inside_function: false,
        is_inside_test: false,
        diagnostics: Vec::new(),
    };
    visitor.visit_body(statements);
    visitor.diagnostics
}

fn is_test_file_name(file_name: &str) -> bool {
    file_name.ends_with("_test.py")
        || (file_name.starts_with("test_") && file_name.ends_with(".py"))
}

// A leading docstring describes the handler rather than doing anything, so it never decides
// whether the body swallows.
fn get_body_after_docstring(handler: &ExceptHandler) -> &[Stmt] {
    let ExceptHandler::ExceptHandler(handler) = handler;
    match handler.body.as_slice() {
        [first, rest @ ..] if is_docstring_stmt(first) => rest,
        statements => statements,
    }
}

fn check_handler(handler: &ExceptHandler) -> Option<Diagnostic> {
    const EXCEPT_LENGTH: TextSize = TextSize::new(6);

    if !get_body_after_docstring(handler)
        .iter()
        .all(is_swallowing_statement)
    {
        return None;
    }

    let ExceptHandler::ExceptHandler(handler) = handler;
    let start = handler.range.start();
    Some(Diagnostic {
        message: "Test swallows the exception, so it cannot fail; let it propagate, or use pytest.raises or a skip condition for the expected case".to_owned(),
        range: handler.type_.as_ref().map_or_else(
            || TextRange::at(start, EXCEPT_LENGTH),
            |exception| TextRange::new(start, exception.end()),
        ),
        noqa_offset: Some(start),
    })
}

// Each of these keeps the caught exception from failing the test.
fn is_swallowing_statement(statement: &Stmt) -> bool {
    if is_inert_statement(statement) {
        return true;
    }
    match statement {
        Stmt::Expr(statement) => match statement.value.as_ref() {
            Expr::Call(call) => is_skip_call(&call.func),
            _ => false,
        },
        _ => false,
    }
}

fn is_inert_handler(handler: &ExceptHandler) -> bool {
    get_body_after_docstring(handler)
        .iter()
        .all(is_inert_statement)
}

fn is_inert_statement(statement: &Stmt) -> bool {
    match statement {
        Stmt::Pass(_) => true,
        Stmt::Return(statement) => statement.value.is_none(),
        Stmt::Expr(statement) => matches!(statement.value.as_ref(), Expr::EllipsisLiteral(_)),
        _ => false,
    }
}

fn is_skip_call(function: &Expr) -> bool {
    let Expr::Attribute(attribute) = function else {
        return false;
    };
    let Expr::Name(name) = attribute.value.as_ref() else {
        return false;
    };
    matches!(
        (name.id.as_str(), attribute.attr.as_str()),
        ("pytest", "skip") | ("self" | "cls", "skipTest")
    )
}

struct SwallowingHandlerVisitor {
    is_inside_function: bool,
    is_inside_test: bool,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Visitor<'a> for SwallowingHandlerVisitor {
    fn visit_stmt(&mut self, statement: &'a Stmt) {
        let previous_is_inside_function = self.is_inside_function;
        let previous_is_inside_test = self.is_inside_test;
        match statement {
            // Collection never descends into a function's body, so the lexical gate treats
            // nothing under one as a test however it is named.
            Stmt::FunctionDef(definition) => {
                self.is_inside_test |=
                    !self.is_inside_function && definition.name.starts_with("test");
                self.is_inside_function = true;
            }
            // An `else` on the `try` is where the hand-rolled assertRaises idiom puts its failure,
            // so a handler that does nothing is exempt there. The exemption does not depend on
            // what the `else` body contains, and a handler that skips is still a finding, since
            // the skip path never reaches the `else`.
            Stmt::Try(try_statement) if self.is_inside_test => {
                let handlers = try_statement.handlers.iter().filter(|handler| {
                    try_statement.orelse.is_empty() || !is_inert_handler(handler)
                });
                self.diagnostics.extend(handlers.filter_map(check_handler));
            }
            _ => {}
        }
        walk_stmt(self, statement);
        self.is_inside_function = previous_is_inside_function;
        self.is_inside_test = previous_is_inside_test;
    }
}
