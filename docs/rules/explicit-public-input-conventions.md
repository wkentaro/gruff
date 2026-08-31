# explicit-public-input-conventions (GR005)

## What it does

Flags each fixed caller-supplied input to a public module-level function or method that is positional-or-keyword. It accepts and excludes the same input shapes as `explicit-non-public-input-conventions` (GR001): positional-only (`/`) and keyword-only (`*`) inputs are accepted, and implicit method receivers and variadic parameters are excluded.

For this syntactic rule, public definitions are the complement of non-public definitions. They include ordinary names, public names with a trailing underscore, framework or protocol sunder hooks, and system-defined dunder methods; the label does not assert that an interface is documented or exported.

## Why

The reasoning matches GR001: a positional-or-keyword input leaves its calling convention to each call site, while a positional-only or keyword-only declaration makes the convention local, deterministic, and enforced at runtime.

Public definitions carry a separate code because their callers can live outside the repository, so the two scopes are adopted independently. An established library can enable GR001 immediately and schedule GR005 for when its public signatures are reviewed. The `GR` and `ALL` selectors enable both, which suits greenfield projects and completed migrations.

## Example

```diff
-def resize_image(data: bytes, width: int) -> bytes:
+def resize_image(data: bytes, /, *, width: int) -> bytes:
     return resize(data, width=width)
```

## When to suppress

Before enabling the rule on an established library, review public and protocol definitions for downstream compatibility: migrate the signatures that are free to change, and suppress the contracts that are not.

Suppress a definition whose contract must keep accepting both positional and keyword calls, since changing it would break callers outside the repository:

```python
def format_cost_compat(value: float) -> str:  # noqa: GR005 -- contract accepts both call styles
    return f"${value:.2f}"
```

Fix everything else. For a file made entirely of protocol implementations, use a per-file ignore instead of repeating the suppression.
