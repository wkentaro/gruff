# Gruff

Gruff is an opinionated, deterministic maintainability linter for Python. It complements Ruff with project policies that make agent-assisted code easier to understand and review; it does not infer who or what wrote the code.

The first release tests four theses: private inputs are easier to trace when callers name them, private behavior is easier to review when callers supply every value, package initializer manifests are easier to review when every public import path defines `__all__`, and constants are easier to review when uppercase names and `Final` annotations always appear together.

GR001, GR002, GR003, and GR004 are opt-in while those theses are validated. A check with no enabled rules succeeds but warns that it performed no policy analysis.

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

### `package-dunder-all` (GR003)

Flags a package initializer when a successfully completing import path leaves a public binding without `__all__`. The rule covers `__init__.py` and `__init__.pyi`, including bindings in module-level control flow, and reports at most one finding per file. Empty, private-only, type-checking-only, and statically false paths do not require a manifest.

Before → after:

```diff
 from .client import Client
 from .errors import GruffError

+__all__ = ["Client", "GruffError"]
```

### `final-constants` (GR004)

Flags simple-name assignments when an uppercase name and a `Final` annotation do not appear together. The rule applies in module, class, and function scopes, including nested control flow. Enum members, type aliases, chained and unpacking assignments, augmented assignments, loop and context-manager targets, attributes, subscripts, and imports are excluded.

Before → after:

```diff
 from typing import Final

-THUMBNAIL_WIDTH = 512
-image_format: Final = "png"
+THUMBNAIL_WIDTH: Final = 512
+IMAGE_FORMAT: Final = "png"
```

### Exceptions

Suppress a rule on definitions that must follow an external calling convention or intentionally provide a convenience default:

```python
def _format_cost(value: float) -> str:  # noqa: GR001 -- Callable[[float], str]
    return f"${value:.2f}"


def _render(*, value: float, unit: str = ""):  # noqa: GR002 -- optional suffix
    return f"{value}{unit}"


EXTERNAL_NAME = 1  # noqa: GR004 -- public protocol spelling
```

For a dynamic package manifest, suppress GR003 on the reported public binding and state why deterministic source analysis does not apply:

```python
public = load_exports()  # noqa: GR003 -- exec() defines __all__ below
```

Prefer an inline suppression because it keeps the exception next to its reason. For files made entirely of protocol implementations, use a per-file ignore instead.

## Recommended Ruff pairing

Gruff does not duplicate checks that Ruff already provides. These Ruff rules extend the same theses to code Gruff does not cover:

```toml
[tool.ruff.lint]
extend-select = ["ARG", "FBT", "B006", "B008", "PLR2004", "RUF012", "RUF022"]
```

`F401` and `F822` are in Ruff's default rule set; the pairing below assumes they stay enabled.

### Callable inputs (GR001, GR002)

`ARG` flags unused function and method arguments, including arguments on private definitions:

```diff
-def _resize_image(*, data: bytes, width: int, legacy: bool) -> bytes:
+def _resize_image(*, data: bytes, width: int) -> bytes:
     return resize(data, width=width)
```

GR001 makes callers of private callables name every input; `FBT001` and `FBT002` extend that to boolean inputs on public callables:

```diff
-def resize_image(data: bytes, keep_aspect: bool) -> bytes:
+def resize_image(data: bytes, *, keep_aspect: bool) -> bytes:
     return resize(data, keep_aspect=keep_aspect)
```

GR002 removes defaults from private callables; `B006` and `B008` catch shared mutable defaults and import-time call defaults on the public callables that keep theirs:

```diff
-def make_thumbnails(data: bytes, widths: list[int] = []) -> list[bytes]:
+def make_thumbnails(data: bytes, widths: list[int] | None = None) -> list[bytes]:

-def fetch_image(client: Client = Client()) -> bytes:
+def fetch_image(client: Client | None = None) -> bytes:
```

### Package manifests (GR003)

GR003 only requires the manifest to exist. Once it does, `F401` flags re-exports missing from it:

```diff
 from .client import Client
 from .errors import GruffError

-__all__ = ["Client"]
+__all__ = ["Client", "GruffError"]
```

`F822` finds names in the manifest that are not defined:

```diff
-__all__ = ["Client", "GruffErorr"]
+__all__ = ["Client", "GruffError"]
```

`RUF022` sorts static manifests:

```diff
-__all__ = ["GruffError", "Client"]
+__all__ = ["Client", "GruffError"]
```

### Constants (GR004)

`PLR2004` turns magic values into named constants, which GR004 then requires to be uppercase and `Final`:

```diff
+MAX_WIDTH: Final = 4096
+
 def validate_width(width: int) -> None:
-    if width > 4096:
+    if width > MAX_WIDTH:
         raise ValueError(width)
```

`RUF012` applies the same annotation discipline to mutable class attributes, which GR004 excludes:

```diff
 class ThumbnailWriter:
-    formats = ["png", "jpg"]
+    formats: ClassVar[list[str]] = ["png", "jpg"]
```

## Interface

Gruff will follow Ruff's familiar command and diagnostic conventions:

```console
gruff check .
gruff check --select GR001 .
gruff check --select GR002 .
gruff check --select GR003 .
gruff check --select GR004 .
gruff check --select GR001,GR002,GR003,GR004 .
```

Lint findings, including invalid Python syntax, will exit with status 1. Configuration, I/O, and internal failures will exit with status 2.

Gruff does not rewrite source code in the first release.

## Configuration

Gruff reads configuration only from `pyproject.toml`:

```toml
[tool.gruff.lint]
select = ["GR001", "GR002", "GR003", "GR004"]
ignore = []
per-file-ignores = { "callbacks.py" = ["GR001"] }
```

Directory discovery checks `.py`, `.pyi`, and `.pyw` files and respects Git ignore files.

## Distribution

Public releases will use PyPI wheels for Linux x86_64 and aarch64, macOS x86_64 and arm64, and Windows x86_64. Gruff is not published to crates.io.
