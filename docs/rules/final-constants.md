# final-constants (GR004)

## What it does

Flags simple-name assignments when an uppercase name and a `Final` annotation do not appear together. The rule applies in module, class, and function scopes, including nested control flow.

Enum members, type aliases, chained and unpacking assignments, augmented assignments, loop and context-manager targets, attributes, subscripts, and imports are excluded. The analysis is syntactic: it resolves neither import aliases nor dataflow.

## Why

Two spellings compete for the same idea when uppercase names and `Final` annotations drift apart, so a reader cannot tell from a binding whether rebinding it elsewhere is expected. Pairing them gives a constant one spelling that both a human and a type checker read the same way.

`Final` prevents type checkers from accepting rebinding; it does not make mutable contents immutable. A `Final[list[str]]` still permits `append`, so use an immutable value when the contents must not change.

## Example

```diff
 from typing import Final

-THUMBNAIL_WIDTH = 512
-image_format: Final = "png"
+THUMBNAIL_WIDTH: Final = 512
+IMAGE_FORMAT: Final = "png"
```

## When to suppress

Fix the finding by default, in whichever direction the binding calls for: add `Final` to a constant, or lowercase a name that is not one.

Suppress when the spelling follows an external convention the code does not control, such as a name a protocol or framework reads by exact case:

```python
EXTERNAL_NAME = 1  # noqa: GR004 -- public protocol spelling
```
