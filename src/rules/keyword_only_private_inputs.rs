use ruff_python_ast::Stmt;

use super::Diagnostic;
use crate::analysis::classify_private_inputs;
use crate::analysis::find_private_definitions;

pub(crate) const CODE: &str = "GR001";
pub(crate) const NAME: &str = "keyword-only-private-inputs";

pub(crate) fn check(statements: &[Stmt]) -> Vec<Diagnostic> {
    find_private_definitions(statements)
        .into_iter()
        .flat_map(|(definition, is_method)| {
            classify_private_inputs(definition, is_method)
                .into_iter()
                .filter(|input| !input.is_keyword_only)
                .map(|input| Diagnostic {
                    message: format!("Private input `{}` must be keyword-only", input.name),
                    range: input.range,
                })
        })
        .collect()
}
