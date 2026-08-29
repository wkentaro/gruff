use std::path::Path;

use ruff_python_ast::Stmt;

use super::Diagnostic;
use super::explicit_non_public_input_conventions::check_definitions;
use crate::analysis::find_public_definitions;

pub(crate) const CODE: &str = "GR005";
pub(crate) const NAME: &str = "explicit-public-input-conventions";

pub(crate) fn check(_path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    check_definitions(find_public_definitions(statements))
}
