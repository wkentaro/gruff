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

The first release tests four theses: inputs are easier to trace when definitions declare how callers pass them, non-public behavior is easier to review when callers supply every value, package initializer manifests are easier to review when every public import path defines `__all__`, and constants are easier to review when uppercase names and `Final` annotations always appear together.

| Code | Rule | Policy |
| --- | --- | --- |
| GR001 | [`explicit-non-public-input-conventions`](#explicit-non-public-input-conventions-gr001) | Every fixed input to a non-public callable has an explicit calling convention. |
| GR002 | [`required-non-public-inputs`](#required-non-public-inputs-gr002) | Callers supply every fixed input to non-public callables. |
| GR003 | [`package-dunder-all`](#package-dunder-all-gr003) | Every public package import path defines `__all__`. |
| GR004 | [`final-constants`](#final-constants-gr004) | Uppercase names and `Final` annotations appear together. |
| GR005 | [`explicit-public-input-conventions`](#explicit-public-input-conventions-gr005) | Every fixed input to a public callable has an explicit calling convention. |

## Configuration and CLI

Gruff reads configuration only from `pyproject.toml`:

```toml
[tool.gruff]
output-format = "full"

[tool.gruff.lint]
select = ["GR001", "GR002", "GR003", "GR004", "GR005"]
ignore = []
per-file-ignores = { "callbacks.py" = ["GR001"] }
```

`output-format` accepts `full`, `concise`, `json`, or `github`. Rule selectors accept an exact code, the `GR` prefix, or `ALL`; the more specific selector wins when `select` and `ignore` overlap, and `ignore` wins ties.

Command-line options override configuration:

```console
gruff check .
gruff check --select GR001,GR002,GR005 .
gruff check --ignore GR004 .
gruff check --output-format github .
gruff check --config path/to/pyproject.toml .
gruff check --isolated --select GR .
```

Pass files or directories as paths. Directory discovery checks `.py`, `.pyi`, and `.pyw` files and respects Git ignore files. Run `gruff check --help` for the complete command reference.

Lint findings, including invalid Python syntax, exit with status 1. Configuration, I/O, and internal failures exit with status 2. Gruff does not rewrite source code in the first release.

## Rule reference

### `explicit-non-public-input-conventions` (GR001)

Flags each fixed caller-supplied input to a non-public module-level function or method that is positional-or-keyword. Positional-only (`/`) and keyword-only (`*`) inputs declare an explicit calling convention and are accepted; implicit method receivers and variadic parameters are excluded.

Before → after:

```diff
-def _resize_image(data: bytes, width: int) -> bytes:
+def _resize_image(data: bytes, /, *, width: int) -> bytes:
     return resize(data, width=width)

 def make_thumbnail(data: bytes, /) -> bytes:
     return _resize_image(data, width=512)
```

A non-public definition starts with an underscore and does not end with one. This includes `_name` and `__name` spellings; double-leading names are name-mangled in class scope. Ordinary, trailing-underscore, sunder, and dunder definitions are excluded.

### `required-non-public-inputs` (GR002)

Flags each fixed caller-supplied input to a non-public module-level function or method that has a default; implicit method receivers and variadic parameters are excluded.

Before → after:

```diff
-def _resize_image(*, data: bytes, width: int = 512) -> bytes:
+def _resize_image(*, data: bytes, width: int) -> bytes:
     return resize(data, width=width)

 def make_thumbnail(data: bytes, /) -> bytes:
-    return _resize_image(data=data)
+    return _resize_image(data=data, width=512)
```

Choose the input shape before suppressing the rule. If callers never vary a value, remove the input and keep the value inside the non-public definition instead of making every caller repeat it. If callers vary the value, keep the input required and have callers supply it explicitly. Reserve a default and GR002 suppression for meaningful semantic policy that would otherwise be duplicated across callers.

### `package-dunder-all` (GR003)

Flags a package initializer when a successfully completing import path leaves a binding whose name does not start with an underscore without `__all__`. The rule covers `__init__.py` and `__init__.pyi`, including bindings in module-level control flow, and reports at most one finding per file. Empty, underscore-prefixed-only, type-checking-only, and statically false paths do not require a manifest.

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

### `explicit-public-input-conventions` (GR005)

Flags each fixed caller-supplied input to a public module-level function or method that is positional-or-keyword. It accepts and excludes the same input shapes as GR001.

For this syntactic policy, public definitions are the complement of non-public definitions. They include ordinary names, public names with a trailing underscore, framework or protocol sunder hooks, and system-defined dunder methods; the label does not infer whether an interface is documented or exported.

Before → after:

```diff
-def resize_image(data: bytes, width: int) -> bytes:
+def resize_image(data: bytes, /, *, width: int) -> bytes:
     return resize(data, width=width)
```

For established libraries, enable GR001 first. Before enabling GR005, review public and protocol definitions for downstream compatibility; migrate compatible signatures and suppress contracts that must still accept both positional and keyword calls. `GR` and `ALL` enable both rules for greenfield projects and completed migrations.

### Exceptions

Use a positional-only marker when an external contract intentionally accepts positional calls. Suppress GR001 or GR005 only when the contract must accept both positional and keyword calls. Suppress GR002 only when a default centralizes meaningful semantic policy that callers would otherwise duplicate. Suppress GR004 when a binding intentionally follows an external convention:

```python
def _format_cost(value: float, /) -> str:
    return f"${value:.2f}"


def format_cost_compat(value: float) -> str:  # noqa: GR005 -- contract accepts both call styles
    return f"${value:.2f}"


def _fetch(*, url: str, timeout: float = 30.0) -> bytes:  # noqa: GR002 -- service timeout policy
    return fetch(url, timeout=timeout)


EXTERNAL_NAME = 1  # noqa: GR004 -- public protocol spelling
```

For a dynamic package manifest, suppress GR003 on the reported binding whose name does not start with an underscore and state why deterministic source analysis does not apply:

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

### Callable inputs (GR001, GR002, GR005)

`ARG` flags unused function and method arguments, a shape none of GR001, GR002, or GR005 inspects:

```diff
-def _resize_image(*, data: bytes, width: int, legacy: bool) -> bytes:
+def _resize_image(*, data: bytes, width: int) -> bytes:
     return resize(data, width=width)
```

Together, GR001 and GR005 make every definition declare each input as positional-only or keyword-only. `FBT001` and `FBT002` go further for booleans, which stay ambiguous at a call site even when Gruff accepts them as positional-only:

```diff
-def resize_image(data: bytes, keep_aspect: bool) -> bytes:
+def resize_image(data: bytes, /, *, keep_aspect: bool) -> bytes:
     return resize(data, keep_aspect=keep_aspect)
```

GR002 removes defaults from non-public callables; `B006` and `B008` catch shared mutable defaults and import-time call defaults on the public callables that keep theirs:

```diff
-def make_thumbnails(data: bytes, /, *, widths: list[int] = []) -> list[bytes]:
+def make_thumbnails(data: bytes, /, *, widths: list[int] | None = None) -> list[bytes]:

-def fetch_image(*, client: Client = Client()) -> bytes:
+def fetch_image(*, client: Client | None = None) -> bytes:
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
 def validate_width(width: int, /) -> None:
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
