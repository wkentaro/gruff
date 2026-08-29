use std::path::Path;

use ruff_python_ast::Stmt;
use ruff_python_ast::helpers::is_docstring_stmt;
use ruff_text_size::Ranged;

use super::Diagnostic;
use crate::analysis::find_non_public_definitions;

pub(crate) const CODE: &str = "GR006";
pub(crate) const NAME: &str = "no-non-public-docstrings";

pub(crate) fn check(_path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    find_non_public_definitions(statements)
        .into_iter()
        .filter_map(|(definition, _)| {
            let docstring = definition
                .body
                .first()
                .filter(|statement| is_docstring_stmt(statement))?
                .as_expr_stmt()?;
            Some(Diagnostic {
                message: format!(
                    "Remove docstring from non-public definition `{}`; rename it if its purpose is unclear",
                    definition.name
                ),
                range: docstring.value.range(),
                noqa_offset: Some(docstring.value.end()),
            })
        })
        .collect()
}
