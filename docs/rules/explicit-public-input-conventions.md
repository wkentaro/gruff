# explicit-public-input-conventions (GR005)

## What it does

Flags each fixed caller-supplied input to a public module-level function or method that is positional-or-keyword. It accepts and excludes the same input shapes as `explicit-non-public-input-conventions` (GR001): positional-only (`/`) and keyword-only (`*`) inputs are accepted, and implicit method receivers and variadic parameters are excluded.

For this syntactic rule, public definitions are the complement of non-public definitions. They include ordinary names, public names with a trailing underscore, framework or protocol sunder hooks, and system-defined dunder methods; the label does not assert that an interface is documented or exported.

## Why

The reasoning matches GR001: a positional-or-keyword input leaves its calling convention to each call site, while a positional-only or keyword-only declaration makes the convention local, deterministic, and enforced at runtime.

Public definitions carry a separate code so the two scopes are adopted independently. An established library can adopt GR001 after reviewing its non-public calling contracts and schedule GR005 for when its public signatures are reviewed. Either scope can include externally invoked callbacks; the name alone does not establish control over callers. The `GR` and `ALL` selectors enable both, which suits greenfield projects and completed migrations.

## Example

```diff
-def resize_image(data: bytes, width: int) -> bytes:
+def resize_image(data: bytes, /, *, width: int) -> bytes:
     return resize(data, width=width)
```

## When to suppress

Before enabling the rule on an established library, review public and protocol definitions for downstream compatibility: migrate the signatures that are free to change, and suppress the contracts that are not.

When the project controls the calling contract, inspect and update callers together with the signature. For framework callbacks and protocol implementations, first establish the external contract. A passing type check does not prove compatibility: dynamically invoked callbacks may be stored behind `Any` or callable types that do not describe their parameters, hiding incompatible argument passing from static analysis.

After changing `/` or `*` placement, run the affected framework lifecycle or a focused smoke test through its actual registration and dispatch path. Check that the callback ran and produced its expected effect without callback errors; a direct call tailored to the new signature is insufficient. The GR001 rule doc gives a [runnable dispatch example](explicit-non-public-input-conventions.md#when-to-suppress) showing why each restriction can break a callback.

Suppress a definition whose contract must keep accepting both positional and keyword calls, since changing it would break callers outside the repository:

```python
def format_cost_compat(value: float) -> str:  # noqa: GR005 -- contract accepts both call styles
    return f"${value:.2f}"
```

Fix everything else whose calling contract is free to change. Prefer an inline suppression because it keeps the exception next to its reason. Before introducing a per-file ignore for GR005, run the rule without that ignore and audit every GR005 finding in every matched file, including reviewing existing inline exceptions. Use a per-file ignore only when all GR005 findings share the same intentional contract exception. Fix unrelated findings and keep inline suppressions for exceptions with different reasons. A per-file ignore also hides future GR005 findings, even when they are unrelated to the audited exception.
