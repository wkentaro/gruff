# Pin Ruff's parser crates

Ruffhouse uses exact versions of Ruff's internal Python parser and AST crates to match Ruff's accepted syntax and source ranges. Exact pins make builds reproducible and isolate Ruffhouse from unannounced API changes, at the cost of deliberate parser upgrades when Ruff publishes new versions.
