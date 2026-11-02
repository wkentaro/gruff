# keyword-only-non-public-inputs (GR013)

## What it does

Flags non-public function and method parameters that a caller can pass by
position. Put `*` before them so callers must name each one. Implicit method
receivers, `*args`, and `**kwargs` are excluded.

## Why

A call that names each argument shows what every value means at the call site,
and reordering the parameters cannot silently swap two values. Non-public
callers all live inside the project, so making them name their arguments costs
no compatibility.

GR013 is stricter than [GR001](explicit-non-public-input-conventions.md), which
also accepts positional-only parameters. A keyword-only parameter satisfies
both rules. When both are enabled, GR001 would only repeat GR013's findings, so
gruff ignores GR001 and prints a warning.

## Example

Require callers to name the image and the width:

```diff
-def _resize_image(data: bytes, width: int) -> bytes:
+def _resize_image(*, data: bytes, width: int) -> bytes:
     return resize(data, width=width)

 def make_thumbnail(data: bytes, /) -> bytes:
-    return _resize_image(data, 512)
+    return _resize_image(data=data, width=512)
```

### Rule scope

GR013 checks the same definitions as GR001: the
[non-public scope](explicit-non-public-input-conventions.md#rule-scope), minus
the [framework-invoked definitions](explicit-non-public-input-conventions.md#framework-invoked-definitions),
[overrides](explicit-non-public-input-conventions.md#overrides), and
[escaping references](explicit-non-public-input-conventions.md#escaping-references).

## When to suppress

Suppress a definition that another module passes as a callback. Gruff checks
one file at a time, so it cannot see that reference:

```python
# _handlers.py
def _on_event(event):  # noqa: GR013 -- app.py registers it as a positional callback
    ...
```

Adding `*` to such a definition breaks the call that passes arguments by
position. Test the actual registration and dispatch path before changing a
callback's signature.
