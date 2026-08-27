# Gruff

Gruff is an opinionated, deterministic maintainability linter for Python. It complements Ruff with project policies that make agent-assisted code easier to understand and review; it does not infer who or what wrote the code.

The first release tests two theses: private inputs are easier to trace when callers name them, and private behavior is easier to review when callers supply every value.

GR001 and GR002 are opt-in while those theses are validated. A check with no enabled rules succeeds but warns that it performed no policy analysis.

## Recommended Ruff pairing

Gruff does not duplicate checks that Ruff already provides. Enable Ruff's `ARG` rules to flag unused function and method arguments, including arguments on private definitions:

```toml
[tool.ruff.lint]
extend-select = ["ARG"]
```

## Rules

### `keyword-only-private-inputs` (GR001)

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

### `required-private-inputs` (GR002)

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

### Exceptions

Suppress a rule on definitions that must follow an external calling convention or intentionally provide a convenience default:

```python
def _format_cost(value: float) -> str:  # noqa: GR001 -- Callable[[float], str]
    return f"${value:.2f}"


def _render(*, value: float, unit: str = ""):  # noqa: GR002 -- optional suffix
    return f"{value}{unit}"
```

Prefer an inline suppression because it keeps the exception next to its reason. For files made entirely of protocol implementations, use a per-file ignore instead.

## Interface

Gruff will follow Ruff's familiar command and diagnostic conventions:

```console
gruff check .
gruff check --select GR001 .
gruff check --select GR002 .
gruff check --select GR001,GR002 .
```

Lint findings, including invalid Python syntax, will exit with status 1. Configuration, I/O, and internal failures will exit with status 2.

Gruff does not rewrite source code in the first release.

## Configuration

Gruff reads configuration only from `pyproject.toml`:

```toml
[tool.gruff.lint]
select = ["GR001", "GR002"]
ignore = []
per-file-ignores = { "callbacks.py" = ["GR001"] }
```

Directory discovery checks `.py`, `.pyi`, and `.pyw` files and respects Git ignore files.

## Distribution

Public releases will use PyPI wheels for Linux x86_64 and aarch64, macOS x86_64 and arm64, and Windows x86_64. Gruff is not published to crates.io.
