pub(crate) mod explicit_non_public_input_conventions;
pub(crate) mod explicit_public_input_conventions;
pub(crate) mod final_constants;
pub(crate) mod no_non_public_docstrings;
pub(crate) mod no_subsumed_comments;
pub(crate) mod package_dunder_all;
pub(crate) mod required_non_public_inputs;

use ruff_text_size::TextRange;
use ruff_text_size::TextSize;

pub(crate) struct Diagnostic {
    pub(crate) message: String,
    pub(crate) range: TextRange,
    pub(crate) noqa_offset: Option<TextSize>,
}
