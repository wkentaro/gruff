# no-non-public-docstrings (GR006)

## What it does

Flags docstrings on non-public module-level functions and methods. Uses the same
[non-public scope](explicit-non-public-input-conventions.md#rule-scope) as GR001.
Public definitions, including recognized direct package re-exports, are excluded.
So are nested functions, methods of classes defined inside functions, and ordinary
comments.

## Why

A docstring that repeats a function's name adds a second description to maintain.
Prefer a name that states the purpose. Put reasoning that a name cannot explain
in a comment inside the definition.

## Example

Remove the docstring when the name already explains the function:

```diff
 def _load_config(*, path: Path) -> Config:
-    """Load configuration from a path."""
     return Config.parse(path.read_text())
```

### Rule scope

A docstring is a string literal at the start of the body. The finding points at
the literal. Gruff has no autofix.

## When to suppress

Rename a definition if removing its docstring would leave its purpose unclear.

Suppress when an external framework requires the docstring, for example by
reading `__doc__`. Put the suppression after the single-line docstring or the
closing quotes of a multiline docstring:

```python
def _documented_hook() -> None:
    """Required by the framework contract."""  # noqa: GR006 -- inherited documentation contract
```
