# no-non-public-docstrings (GR006)

## What it does

Flags a non-public module-level function or method whose first body statement is a string literal. The rule uses the same non-public definition boundary as `explicit-non-public-input-conventions` (GR001) and `required-non-public-inputs` (GR002).

Public definitions, nested functions, and ordinary comments are outside the rule. The finding points at the docstring literal, and Gruff does not rewrite the source.

## Why

A docstring on a non-public definition usually restates the name, so a reader pays for a second description that no one is obliged to keep true. Carrying the purpose in the name instead keeps one description, and the compiler-visible one at that: a rename moves it, while a stale docstring survives every rename.

If a definition is unclear without its docstring, the name is the thing to fix. Non-obvious reasoning that the name cannot carry belongs in an ordinary comment inside the definition.

## Example

```diff
 def _load_config(*, path: Path) -> Config:
-    """Load configuration from a path."""
     return Config.parse(path.read_text())
```

## When to suppress

Remove a docstring that restates the name, and rename the definition when removing the docstring would leave it unclear.

Suppress when a definition inherits a documentation contract it does not control, such as a framework hook that reads `__doc__`. The suppression belongs on the logical end line of the docstring: after a single-line docstring, or after the closing quotes of a multiline one:

```python
def _documented_hook() -> None:
    """Required by the framework contract."""  # noqa: GR006 -- inherited documentation contract
```
