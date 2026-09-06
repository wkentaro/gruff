# public-attribute-properties (GR012)

## What it does

Flags direct public instance-attribute stores such as `self.name = value`.
The rule accepts underscore-prefixed instance storage when it is used from
that receiver, and accepts an explicit `@property` or property setter as the
public boundary. It reports annotated assignments, ordinary assignments,
augmented assignments, and stores outside `__init__`.

The companion access boundary is already provided by Ruff `SLF001`
(`private-member-access`): it reports `logger._name` while allowing
`logger.name`. Gruff deliberately does not duplicate that check; enable
`SLF001` alongside `GR012` when adopting this policy.

The rule is intentionally syntactic. It follows the first positional argument
of an ordinary class method as its instance receiver, treats `@classmethod`
receivers as class state, and ignores static methods. It does not infer
inheritance or runtime descriptors. Class-level assignments, `ClassVar`, and
class declarations used by dataclass, attrs, or model frameworks are outside
the direct-store check because they do not spell an instance store. Likewise,
inherited properties and `__slots__` declarations are not resolved. An
explicit `self.public = value` in a method remains a direct store even when a
framework or slots declaration exists; suppress it when that framework owns
the contract. A property setter is an explicit boundary, so stores in a
method decorated with `@name.setter` are allowed. The rule does not autofix.

## Why

Public fields silently become contracts: callers can read or replace them
without a controlled compatibility boundary. Keeping the storage spelling
private and exposing a property makes that boundary explicit, while allowing
internal state to remain internal. The separate access check prevents a local
underscore repair from merely moving an external caller onto the backing
field.

## Example

```diff
 class Result:
     def __init__(self) -> None:
-        self.error_message = None
+        self._error_message = None

+    @property
+    def error_message(self):
+        return self._error_message
```

```python
logger._error_message  # SLF001: use the public property instead
```

## When to suppress

Suppress a finding when a framework or protocol requires the spelling, or when
the class deliberately implements a dynamic attribute contract that this
syntactic rule cannot see:

```python
self.error_message = value  # noqa: GR012 -- framework callback attribute
```
