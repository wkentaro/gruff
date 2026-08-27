pub(crate) mod final_constants;
pub(crate) mod keyword_only_private_inputs;
pub(crate) mod package_dunder_all;
pub(crate) mod required_private_inputs;

use ruff_text_size::TextRange;

pub(crate) struct Diagnostic {
    pub(crate) message: String,
    pub(crate) range: TextRange,
}
