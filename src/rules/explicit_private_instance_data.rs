use std::path::Path;

use ruff_python_ast::Stmt;

use super::Diagnostic;
use super::explicit_instance_data::find_undeclared_stores;
use crate::analysis::is_non_public_name;

pub(crate) const CODE: &str = "GR015";
pub(crate) const NAME: &str = "explicit-private-instance-data";
pub(crate) const SUMMARY: &str =
    "Private instance data is declared through class-body annotations.";

pub(crate) fn check(_path: &Path, statements: &[Stmt]) -> Vec<Diagnostic> {
    find_undeclared_stores(statements, is_non_public_name)
        .into_iter()
        .map(|store| Diagnostic {
            message: if store.is_property {
                format!(
                    "Write to getter-only property `{}`; declare a setter for intentional writes.",
                    store.name
                )
            } else {
                format!(
                    "Private instance data field `{}` is implicit; declare it with an annotation in the class body.",
                    store.name
                )
            },
            range: store.range,
            noqa_offset: None,
        })
        .collect()
}
