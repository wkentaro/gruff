# final-constants (GR004)

## What it does

Flags a simple-name assignment when an uppercase name lacks `Final`, or a `Final`
annotation has a name that is not uppercase. Applies in module, class, and
function scopes, including nested control flow.

## Why

An uppercase name marks a constant for readers. `Final` lets a type checker
reject rebinding. Using both makes the intent consistent.

`Final` does not freeze mutable contents. A `Final[list[str]]` still permits
`append`; use an immutable value if the contents must not change.

## Example

Add the missing annotation or uppercase the name. Update references to any
renamed binding too:

```diff
 from typing import Final

-THUMBNAIL_WIDTH = 512
-image_format: Final = "png"
+THUMBNAIL_WIDTH: Final = 512
+IMAGE_FORMAT: Final = "png"
```

### Rule scope

Enum members, type aliases, chained and unpacking assignments, augmented
assignments, loop and context-manager targets, attributes, subscripts, and
imports are excluded. Gruff checks syntax without resolving import aliases or
dataflow.

## When to suppress

Add `Final` to a constant, or lowercase a name that is not one.

Suppress a spelling required by an external protocol or framework. Place the
reason beside the assigned name, even when the value spans multiple lines:

```python
EXTERNAL_NAMES = (  # noqa: GR004 -- public protocol spelling
    "one",
)
```
