# Ruffhouse

Ruffhouse is an opinionated, deterministic maintainability linter for Python. It complements Ruff with project policies that make agent-assisted code easier to understand and review; it does not infer who or what wrote the code.

The first release tests two theses: private inputs are easier to trace when callers name them, and private behavior is easier to review when callers supply every value.

RH001 and RH002 are opt-in while those theses are validated. A check with no enabled rules succeeds but warns that it performed no policy analysis.

## Rules

### `keyword-only-private-inputs` (RH001)

Flags each fixed caller-supplied input to a private module-level function or method that is positional; implicit method receivers and variadic parameters are excluded.

Before → after:

```diff
-def _resize_image(data: bytes, width: int) -> bytes:
+def _resize_image(*, data: bytes, width: int) -> bytes:
     return resize(data, width=width)

 def make_thumbnail(data: bytes) -> bytes:
-    return _resize_image(data, width=512)
+    return _resize_image(data=data, width=512)
```

### `required-private-inputs` (RH002)

Flags each fixed caller-supplied input to a private module-level function or method that has a default; implicit method receivers and variadic parameters are excluded.

Before → after:

```diff
-def _resize_image(*, data: bytes, width: int = 512) -> bytes:
+def _resize_image(*, data: bytes, width: int) -> bytes:
     return resize(data, width=width)

 def make_thumbnail(data: bytes) -> bytes:
-    return _resize_image(data=data)
+    return _resize_image(data=data, width=512)
```

## Interface

Ruffhouse will follow Ruff's familiar command and diagnostic conventions:

```console
ruffhouse check .
ruffhouse check --select RH001 .
ruffhouse check --select RH002 .
ruffhouse check --select RH001,RH002 .
```

Lint findings, including invalid Python syntax, will exit with status 1. Configuration, I/O, and internal failures will exit with status 2.

Ruffhouse does not rewrite source code in the first release.

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
