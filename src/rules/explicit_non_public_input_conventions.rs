use std::path::Path;

use ruff_python_ast::Stmt;
use ruff_python_ast::StmtFunctionDef;

use super::Diagnostic;
use crate::analysis::classify_inputs;
use crate::analysis::find_non_public_definitions;

pub(crate) const CODE: &str = "GR001";
pub(crate) const NAME: &str = "explicit-non-public-input-conventions";

pub(crate) fn check(_path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    check_definitions(find_non_public_definitions(statements))
}

pub(super) fn check_definitions(definitions: Vec<(&StmtFunctionDef, bool)>) -> Vec<Diagnostic> {
    definitions
        .into_iter()
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
                })
        })
        .collect()
}
