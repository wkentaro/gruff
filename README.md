# Gruff

Gruff is an opinionated, deterministic maintainability linter for Python. It complements Ruff with project policies that make agent-assisted code easier to understand and review; it does not infer who or what wrote the code.

## Installation

Requires Python 3.10 or later.

```bash
pip install gruff
```

Or with [uv](https://docs.astral.sh/uv/):

```bash
uv tool install gruff
```

Verify it works:

```bash
gruff --version
```

> [!TIP]
> To try the latest development version (the head of `main` on GitHub) before
> it is published:
>
> ```bash
> uv tool install git+https://github.com/wkentaro/gruff
> ```

## Quick start

Enable every Gruff rule in `pyproject.toml`:

```toml
[tool.gruff.lint]
select = ["GR"]
```

Then check the current directory:

```bash
gruff check .
```

All rules are opt-in. Use an exact code such as `GR001` to adopt rules individually; `GR` enables every Gruff rule. A check with no enabled rules succeeds but warns that it performed no policy analysis.

## Rules at a glance

The first release tests four theses: private inputs are easier to trace when definitions declare how callers pass them, private behavior is easier to review when callers supply every value, package initializer manifests are easier to review when every public import path defines `__all__`, and constants are easier to review when uppercase names and `Final` annotations always appear together.

| Code | Rule | Policy |
| --- | --- | --- |
| GR001 | [`keyword-only-private-inputs`](#keyword-only-private-inputs-gr001) | Every fixed private input has an explicit calling convention. |
| GR002 | [`required-private-inputs`](#required-private-inputs-gr002) | Callers supply every fixed input to private callables. |
| GR003 | [`package-dunder-all`](#package-dunder-all-gr003) | Every public package import path defines `__all__`. |
| GR004 | [`final-constants`](#final-constants-gr004) | Uppercase names and `Final` annotations appear together. |

## Configuration and CLI

Gruff reads configuration only from `pyproject.toml`:

```toml
[tool.gruff]
output-format = "full"

[tool.gruff.lint]
select = ["GR001", "GR002", "GR003", "GR004"]
ignore = []
per-file-ignores = { "callbacks.py" = ["GR001"] }
```

`output-format` accepts `full`, `concise`, `json`, or `github`. Rule selectors accept an exact code, the `GR` prefix, or `ALL`; the more specific selector wins when `select` and `ignore` overlap, and `ignore` wins ties.

Command-line options override configuration:

```console
gruff check .
gruff check --select GR001,GR002 .
gruff check --ignore GR004 .
gruff check --output-format github .
gruff check --config path/to/pyproject.toml .
gruff check --isolated --select GR .
```

Pass files or directories as paths. Directory discovery checks `.py`, `.pyi`, and `.pyw` files and respects Git ignore files. Run `gruff check --help` for the complete command reference.

Lint findings, including invalid Python syntax, exit with status 1. Configuration, I/O, and internal failures exit with status 2. Gruff does not rewrite source code in the first release.

## Rule reference

### `keyword-only-private-inputs` (GR001)

Flags each fixed caller-supplied input to a private module-level function or method that is positional-or-keyword. Positional-only (`/`) and keyword-only (`*`) inputs declare an explicit calling convention and are accepted; implicit method receivers and variadic parameters are excluded.

Before → after:

```diff
-def _resize_image(data: bytes, width: int) -> bytes:
+def _resize_image(data: bytes, /, *, width: int) -> bytes:
     return resize(data, width=width)

 def make_thumbnail(data: bytes) -> bytes:
     return _resize_image(data, width=512)
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

Choose the input shape before suppressing the rule. If callers never vary a value, remove the input and keep the value inside the private definition instead of making every caller repeat it. If callers vary the value, keep the input required and have callers supply it explicitly. Reserve a default and GR002 suppression for meaningful semantic policy that would otherwise be duplicated across callers.

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

`Final` prevents type checkers from accepting rebinding; it does not make mutable contents immutable. For example, a `Final[list[str]]` still permits `append`. Use an immutable value when the contents must not change.

### Exceptions

Use a positional-only marker when an external contract intentionally accepts positional calls. Suppress GR001 only when the contract must accept both positional and keyword calls. Suppress GR002 only when a default centralizes meaningful semantic policy that callers would otherwise duplicate. Suppress GR004 when a binding intentionally follows an external convention:

```python
def _format_cost(value: float, /) -> str:
    return f"${value:.2f}"


def _format_cost_compat(value: float) -> str:  # noqa: GR001 -- contract accepts both call styles
    return f"${value:.2f}"


def _fetch(*, url: str, timeout: float = 30.0) -> bytes:  # noqa: GR002 -- service timeout policy
    return fetch(url, timeout=timeout)


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

GR001 makes private definitions declare each input as positional-only or keyword-only; `FBT001` and `FBT002` extend the keyword-only convention to boolean inputs on public callables:

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

## Distribution

Gruff releases use PyPI wheels for Linux x86_64 and aarch64, macOS x86_64 and arm64, and Windows x86_64. Gruff is not published to crates.io.
