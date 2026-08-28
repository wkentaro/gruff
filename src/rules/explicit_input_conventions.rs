use std::path::Path;

use ruff_python_ast::Stmt;

use super::Diagnostic;
use crate::analysis::classify_inputs;
use crate::analysis::find_definitions;

pub(crate) const CODE: &str = "GR001";
pub(crate) const NAME: &str = "explicit-input-conventions";

pub(crate) fn check(_path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    find_definitions(statements)
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
