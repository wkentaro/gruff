# required-non-public-inputs (GR002)

## What it does

Flags each fixed caller-supplied input to a non-public module-level function or method that has a default. Implicit method receivers and variadic parameters are excluded.

The rule uses the same non-public definition boundary as `explicit-non-public-input-conventions` (GR001): a name that starts with an underscore and does not end with one, including `_name` and `__name` spellings.

## Why

A default hides part of a non-public definition's behavior at the call site, so a reader has to open the definition to learn which value a caller actually gets. Requiring callers to supply every value makes the behavior of non-public code readable without caller analysis, and it keeps the default from silently drifting away from what any single caller expects.

## Example

```diff
-def _resize_image(*, data: bytes, width: int = 512) -> bytes:
+def _resize_image(*, data: bytes, width: int) -> bytes:
     return resize(data, width=width)

 def make_thumbnail(data: bytes, /) -> bytes:
-    return _resize_image(data=data)
+    return _resize_image(data=data, width=512)
```

## When to suppress

Choose the input shape before suppressing the rule. If callers never vary a value, remove the input and keep the value inside the non-public definition instead of making every caller repeat it. If callers vary the value, keep the input required and have callers supply it explicitly.

Reserve a default and a suppression for a default that centralizes meaningful semantic policy which callers would otherwise duplicate:

```python
def _fetch(*, url: str, timeout: float = 30.0) -> bytes:  # noqa: GR002 -- service timeout policy
    return fetch(url, timeout=timeout)
```
