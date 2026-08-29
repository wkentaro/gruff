pub(crate) mod explicit_non_public_input_conventions;
pub(crate) mod explicit_public_input_conventions;
pub(crate) mod final_constants;
pub(crate) mod package_dunder_all;
pub(crate) mod required_non_public_inputs;

use ruff_text_size::TextRange;

pub(crate) struct Diagnostic {
    pub(crate) message: String,
    pub(crate) range: TextRange,
}
