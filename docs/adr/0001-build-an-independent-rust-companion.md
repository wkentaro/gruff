# Build an independent Rust companion

Ruffhouse is a standalone Rust binary rather than a Ruff plugin, Ruff fork, or Python package. Ruff has no stable third-party rule interface, while an independent binary keeps Ruffhouse's policies separately versioned and avoids depending on the checked project's Python environment.
