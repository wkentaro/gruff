# explicit-non-public-input-conventions (GR001)

## What it does

Flags each fixed caller-supplied input to a non-public module-level function or method that is positional-or-keyword. Positional-only (`/`) and keyword-only (`*`) inputs declare an explicit calling convention and are accepted; implicit method receivers and variadic parameters are excluded.

A non-public definition starts with an underscore and does not end with one. This includes `_name` and `__name` spellings; double-leading names are name-mangled in class scope. Ordinary, trailing-underscore, sunder, and dunder definitions are outside the rule. Nested functions and methods of a class defined inside a function are also outside the rule.

## Why

A positional-or-keyword input leaves its calling convention to each call site, so a reader has to collect the callers before knowing how the input is passed. Positional-only and keyword-only declarations make the convention local, deterministic, and enforced at runtime.

Non-public definitions adopt the policy first because every caller lives in the same repository, so the migration has no downstream cost. `explicit-public-input-conventions` (GR005) carries the same policy to the complementary public bucket, and the two rules partition every definition without inferring whether an interface is documented or exported.

## Example

```diff
-def _resize_image(data: bytes, width: int) -> bytes:
+def _resize_image(data: bytes, /, *, width: int) -> bytes:
     return resize(data, width=width)

 def make_thumbnail(data: bytes, /) -> bytes:
     return _resize_image(data, width=512)
```

## When to suppress

Fix the finding by default. Adding `/` or `*` to a non-public definition is a local edit, and every caller is in the same repository. Choose positional-only for a value whose role the name already carries, and keyword-only for the rest.

Suppress only when the definition is bound to an external contract that must keep accepting both positional and keyword calls:

```python
def _handler(request: Request, context: Context) -> Response:  # noqa: GR001 -- runtime invokes both call styles
    return respond(request, context)
```

Prefer an inline suppression because it keeps the exception next to its reason. For a file made entirely of such contracts, use a per-file ignore instead.
