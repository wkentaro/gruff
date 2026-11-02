use std::path::Path;

use ruff_python_ast::Stmt;

use super::Diagnostic;
use super::explicit_non_public_input_conventions::is_framework_invoked;
use crate::analysis::classify_inputs;
use crate::analysis::find_non_public_definitions;
use crate::analysis::is_test_file;

pub(crate) const CODE: &str = "GR013";
pub(crate) const NAME: &str = "keyword-only-non-public-inputs";
pub(crate) const SUMMARY: &str = "Every fixed input to a non-public callable is keyword-only.";

pub(crate) fn check(path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    let is_in_test_file = is_test_file(path);
    find_non_public_definitions(path, statements)
        .into_iter()
        .filter(|(definition, _)| !is_framework_invoked(definition, is_in_test_file))
        .flat_map(|(definition, is_method)| {
            classify_inputs(definition, is_method)
                .into_iter()
                .filter(|input| !input.is_keyword_only)
                .map(|input| Diagnostic {
                    message: format!("Input `{}` must be keyword-only", input.name),
                    range: input.range,
                    noqa_offset: None,
                })
        })
        .collect()
}
