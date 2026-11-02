use std::path::Path;

use ruff_python_ast::Expr;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtFunctionDef;

use super::Diagnostic;
use crate::analysis::classify_inputs;
use crate::analysis::find_non_public_definitions;
use crate::analysis::is_test_file;

pub(crate) const CODE: &str = "GR001";
pub(crate) const NAME: &str = "explicit-non-public-input-conventions";
pub(crate) const SUMMARY: &str =
    "Every fixed input to a non-public callable has an explicit calling convention.";

pub(crate) fn check(path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    check_definitions(path, find_non_public_definitions(path, statements))
}

pub(super) fn check_definitions(
    path: &Path,
    definitions: Vec<(&StmtFunctionDef, bool)>,
) -> Vec<Diagnostic> {
    let is_in_test_file = is_test_file(path);
    definitions
        .into_iter()
        .filter(|(definition, _)| !is_framework_invoked(definition, is_in_test_file))
        .flat_map(|(definition, is_method)| {
            classify_inputs(definition, is_method)
                .into_iter()
                .filter(|input| input.is_positional_or_keyword)
                .map(|input| Diagnostic {
                    message: format!(
                        "Input `{}` must be positional-only or keyword-only",
                        input.name
                    ),
                    range: input.range,
                    noqa_offset: None,
                })
        })
        .collect()
}

// A framework calls these through a registration, not from a call site a reader can search, so
// the framework already decides how each input is passed.
const REGISTRATION_DECORATORS: &[&str] = &[
    // pytest
    "fixture",
    // click and typer
    "command",
    "group",
    "callback",
    // pydantic
    "field_validator",
    "field_serializer",
    "model_validator",
    "model_serializer",
    "validator",
    "root_validator",
    // FastAPI, Flask, and Starlette routes
    "get",
    "post",
    "put",
    "patch",
    "delete",
    "head",
    "options",
    "route",
    "api_route",
    "websocket",
];

pub(super) fn is_framework_invoked(definition: &StmtFunctionDef, is_in_test_file: bool) -> bool {
    (is_in_test_file && definition.name.starts_with("test"))
        // pluggy calls a pytest hook implementation by keyword, matching it by this prefix.
        || definition.name.starts_with("pytest_")
        || definition.decorator_list.iter().any(|decorator| {
            get_decorator_name(&decorator.expression)
                .is_some_and(|name| REGISTRATION_DECORATORS.contains(&name))
        })
}

fn get_decorator_name(expression: &Expr) -> Option<&str> {
    match expression {
        Expr::Call(call) => get_decorator_name(&call.func),
        Expr::Attribute(attribute) => Some(attribute.attr.as_str()),
        Expr::Name(name) => Some(name.id.as_str()),
        _ => None,
    }
}
