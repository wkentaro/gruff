# package-dunder-all (GR003)

## What it does

Flags a package initializer that binds a name without a leading `_` but does not
bind `__all__` on every successfully completing import path. Covers `__init__.py`
and `__init__.pyi`, with at most one finding per file.

## Why

`__all__` tells readers which names a package exports. Without it, an imported
name could be a re-export or an implementation detail.

Gruff checks that `__all__` exists. Ruff checks its contents with `F401` for
unlisted imports, `F822` for undefined names, and `RUF022` for sorting.

## Example

In `__init__.py`, list the names the package exports:

```diff
 from .client import Client
 from .errors import GruffError

+__all__ = ["Client", "GruffError"]
```

### Rule scope

Bindings inside module-level control flow count. Empty, underscore-prefixed-only,
type-checking-only, and statically false paths do not need `__all__`. Gruff
analyzes the source without importing or executing the package.

## When to suppress

Add `__all__` for exports you control. Suppress when a package builds it
dynamically and source analysis cannot see the declaration. Put the reason on
the reported binding:

```python
public = load_exports()  # noqa: GR003 -- exec() defines __all__ below
```
