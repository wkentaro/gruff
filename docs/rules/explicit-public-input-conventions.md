# explicit-public-input-conventions (GR005)

## What it does

Flags public function and method parameters that accept both positional and
keyword arguments. Use `/` for positional-only parameters and `*` for keyword-only
parameters. Implicit method receivers, `*args`, and `**kwargs` are excluded.

## Why

An explicit signature tells readers how to call a function without searching its
call sites. Python enforces that convention at runtime.

Public signatures may have callers outside your project. GR005 is separate from
[GR001](explicit-non-public-input-conventions.md) so you can adopt the non-public
scope first and review public contracts separately. The `GR` and `ALL` selectors
enable both rules.

## Example

Keep image data positional and require the caller to name the width. Update
callers that use another form:

```diff
-def resize_image(data: bytes, width: int) -> bytes:
+def resize_image(data: bytes, /, *, width: int) -> bytes:
     return resize(data, width=width)
+
+
-image = resize_image(data=source, width=512)
+image = resize_image(source, width=512)
```

Parameters before `/` accept positional arguments only. Parameters after `*`
accept keyword arguments only.

### Rule scope

GR005 covers the definitions outside GR001's
[non-public scope](explicit-non-public-input-conventions.md#rule-scope). These
include ordinary names in public scopes, trailing-underscore names, sunder hooks,
dunder methods, and the direct package re-exports recognized by GR001. The
classification uses spelling, scope, and recognized exports; it does not require
API documentation. Nested functions and methods of classes defined inside
functions are excluded.

## When to suppress

Before enabling GR005 on an existing library, review public and protocol
signatures for downstream compatibility. Migrate the signatures you can change.
Suppress contracts that must keep accepting both call styles:

```python
def format_cost_compat(value: float) -> str:  # noqa: GR005 -- contract accepts both call styles
    return f"${value:.2f}"
```

For framework callbacks, test the actual registration and dispatch path. Check
that the callback ran and produced its expected result without errors. A passing
type check or a direct call tailored to the new signature is insufficient. The
GR001 doc has a [runnable example](explicit-non-public-input-conventions.md#when-to-suppress)
where either restriction breaks dispatch.

Prefer inline suppressions with a reason. Before adding a per-file ignore, run
the rule without it and audit every GR005 finding in every matched file, including
existing inline exceptions. Use a per-file ignore only when all findings share
the same intentional contract exception. Fix unrelated findings and keep inline
suppressions for exceptions with different reasons. A per-file ignore also hides
future findings that may have nothing to do with the audited exception.
