# Configuration

Gruff reads configuration only from `pyproject.toml`:

```toml
[tool.gruff]
output-format = "full"

[tool.gruff.lint]
select = ["GR001", "GR002", "GR003", "GR004", "GR005", "GR006"]
ignore = []
per-file-ignores = { "callbacks.py" = ["GR001"] }
```

`output-format` accepts `full`, `concise`, `json`, or `github`. Rule selectors accept an exact code, the `GR` prefix, or `ALL`; the more specific selector wins when `select` and `ignore` overlap, and `ignore` wins ties.

## Command line

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

To read a rule without leaving the terminal:

```console
gruff rule GR004
gruff rule final-constants
gruff rule --all
gruff rule GR004 --output-format json
```

The text output is the rule doc verbatim; the JSON output adds the rule code, name, and one-line policy around it. An unknown rule exits with status 2.

## Suppressing findings

Every rule doc states when to fix a finding and when to suppress it. Suppress with an inline `# noqa` comment carrying the rule code and the reason:

```python
def format_cost_compat(value: float) -> str:  # noqa: GR005 -- contract accepts both call styles
    return f"${value:.2f}"
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
