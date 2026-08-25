# Ruffhouse

Ruffhouse is an opinionated, deterministic maintainability linter for Python. It complements Ruff with project policies that make agent-assisted code easier to understand and review; it does not infer who or what wrote the code.

The project is currently a design scaffold. No lint rules are implemented yet.

The first release will test one thesis: single-use private call wrappers that add no meaningful boundary make agent-written code harder to follow.

RH001 is opt-in while that thesis is validated. A check with no enabled rules succeeds but warns that it performed no policy analysis.

## Interface

Ruffhouse will follow Ruff's familiar command and diagnostic conventions:

```console
ruffhouse check .
ruffhouse check --select RH001 .
```

Lint findings, including invalid Python syntax, will exit with status 1. Configuration, I/O, and internal failures will exit with status 2.

Human-readable findings point to both the private wrapper definition and its sole caller. Ruffhouse does not rewrite source code in the first release.

## Configuration

Ruffhouse reads configuration only from `pyproject.toml`:

```toml
[tool.ruffhouse.lint]
select = ["RH001"]
ignore = []
```

Directory discovery checks `.py`, `.pyi`, and `.pyw` files and respects Git ignore files.

## Distribution

Public releases will use PyPI wheels for Linux x86_64 and aarch64, macOS x86_64 and arm64, and Windows x86_64. Ruffhouse is not published to crates.io.
