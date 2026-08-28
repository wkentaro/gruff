use std::path::Path;

use ruff_python_ast::Stmt;

use super::Diagnostic;
use crate::analysis::classify_inputs;
use crate::analysis::find_private_definitions;

pub(crate) const CODE: &str = "GR002";
pub(crate) const NAME: &str = "required-private-inputs";

pub(crate) fn check(_path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    find_private_definitions(statements)
        .into_iter()
        .flat_map(|(definition, is_method)| {
            classify_inputs(definition, is_method)
                .into_iter()
                .filter(|input| !input.is_required)
                .map(|input| Diagnostic {
                    message: format!("Private input `{}` must be required", input.name),
                    range: input.range,
                })
        })
        .collect()
}
