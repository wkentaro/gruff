use ruff_python_ast::StmtFunctionDef;

use super::Diagnostic;
use crate::analysis::classify_private_inputs;

pub(crate) const CODE: &str = "RH001";
pub(crate) const NAME: &str = "keyword-only-private-inputs";

pub(crate) fn check(definition: &StmtFunctionDef, is_method: bool) -> Vec<Diagnostic> {
    classify_private_inputs(definition, is_method)
        .into_iter()
        .filter(|input| !input.is_keyword_only)
        .map(|input| Diagnostic {
            message: format!("Private input `{}` must be keyword-only", input.name),
            range: input.range,
        })
        .collect()
}
