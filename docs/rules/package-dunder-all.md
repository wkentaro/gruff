# package-dunder-all (GR003)

## What it does

Flags a package initializer when a successfully completing import path leaves a binding whose name does not start with an underscore without `__all__`. The rule covers `__init__.py` and `__init__.pyi`, including bindings in module-level control flow, and reports at most one finding per file.

Empty, underscore-prefixed-only, type-checking-only, and statically false paths do not require a manifest. The analysis is module-local: it never imports or executes the checked package.

## Why

A package initializer without `__all__` leaves its public surface implicit, so a reader has to guess which imported names are re-exports and which are incidental. An explicit manifest states the surface once, in the file that owns it.

The rule only requires the manifest to exist; its contents stay with Ruff. `F401` then flags re-exports missing from the manifest, `F822` finds manifest names that are not defined, and `RUF022` sorts it.

## Example

```diff
 from .client import Client
 from .errors import GruffError

+__all__ = ["Client", "GruffError"]
```

## When to suppress

Fix the finding by default: adding the manifest is a one-line edit that also unlocks the Ruff rules above.

Suppress only when a package builds its manifest dynamically, so deterministic source analysis cannot see it. Place the suppression on the reported binding whose name does not start with an underscore and state why:

```python
public = load_exports()  # noqa: GR003 -- exec() defines __all__ below
```
