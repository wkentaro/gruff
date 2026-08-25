# Ruffhouse

Ruffhouse is an opinionated, deterministic maintainability linter for Python. It complements Ruff with project policies that make agent-assisted code easier to understand and review; it does not infer who or what wrote the code.

The first release tests two theses: single-use private call wrappers add needless indirection, and private definitions are easier to review when every fixed input is required and keyword-only.

RH001 and RH002 are opt-in while those theses are validated. A check with no enabled rules succeeds but warns that it performed no policy analysis.

## Interface

Ruffhouse will follow Ruff's familiar command and diagnostic conventions:

```console
ruffhouse check .
ruffhouse check --select RH001 .
ruffhouse check --select RH002 .
```

Lint findings, including invalid Python syntax, will exit with status 1. Configuration, I/O, and internal failures will exit with status 2.

Human-readable findings point to both the private wrapper definition and its sole caller. Ruffhouse does not rewrite source code in the first release.

RH001 flags a private module-level function only when its body is one direct delegated call, optionally preceded by one call-free local binding, and its sole caller directly returns, awaits, or discards that call. Calls nested inside another expression are excluded.

RH002 flags a private module-level function or method when any fixed caller-supplied input is positional or has a default. Implicit method receivers and variadic parameters are excluded.

## Configuration

Ruffhouse reads configuration only from `pyproject.toml`:

```toml
[tool.ruffhouse.lint]
select = ["RH001", "RH002"]
ignore = []
```

Directory discovery checks `.py`, `.pyi`, and `.pyw` files and respects Git ignore files.

## Distribution

Public releases will use PyPI wheels for Linux x86_64 and aarch64, macOS x86_64 and arm64, and Windows x86_64. Ruffhouse is not published to crates.io.
