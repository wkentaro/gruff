# explicit-private-instance-data (GR015)

## What it does

Flags private instance attributes assigned in a method without an annotation in
the same class body. A private name starts with `_` and does not end with one,
so dunder names such as `__dict__` and sunder names such as `_value_` are not
checked. GR012 checks public names the same way, so the two rules never report
the same store.

An annotation on `self._value` inside a method does not declare the field.

## Why

Without a declaration, a type checker infers the attribute's type from every
value assigned to it. If `__init__` sets `self._ready = False` and another method
sets `self._ready = "yes"`, ty infers `bool | str` and reports nothing. With
`_ready: bool` in the class body, the second assignment is an error.

Even a single assignment leaves room for error. ty infers
`self._lock = threading.Lock()` as `Unknown | LockType`, so it accepts
`self._lock = None` later in the class.

## Example

Move the annotation to the class body, or add one for a plain assignment:

```diff
 class Backend:
+    _timeout: float | None
+    _lock: threading.Lock
+
     def __init__(self, timeout: float | None) -> None:
-        self._timeout: float | None = timeout
+        self._timeout = timeout
         self._lock = threading.Lock()
```

A class-body annotation runs when Python defines the class, unlike one inside a
method. On Python 3.13 and earlier, quote an annotation that names something
defined later in the module, or add `from __future__ import annotations`:

```diff
 class Backend:
+    _process: "_Process | None"
+
     def __init__(self) -> None:
-        self._process: _Process | None = None
+        self._process = None


 _Process = subprocess.Popen | multiprocessing.Process
```

A bare `Final` inside a method takes its type from the value. In the class
body, give the type:

```diff
 class Runtime:
+    _device: Final[str | None]
+
     def __init__(self, device: str | None) -> None:
-        self._device: Final = device
+        self._device = device
```

### Rule scope

GR015 is opt-in and has no autofix. Gruff does not infer the type to declare.

- It checks the same stores as
  [GR012](explicit-instance-data.md#rule-scope): direct attribute targets on
  an instance method's first positional parameter, in any method and any
  control flow.
- Private names include name-mangled ones. `__value: int` in the class body
  declares `self.__value`, since Python mangles both the same way.
- The declaration rules match GR012's. An annotation directly in the same class
  body declares the field, with or without a default. `ClassVar` and `InitVar`
  annotations, unannotated defaults, and `__slots__` entries do not. An
  annotation without a default works alongside `__slots__`.
- A recognized setter permits writes to a private property, and a getter-only
  private property rejects them, as in GR012.
- A declaration does not show that anything assigns the field. ty accepts a
  read of a declared attribute that no code has stored yet, so neither ty nor
  GR015 catches a read before the first store.

## When to suppress

Gruff does not resolve inherited declarations. If a base class declares the
field, suppress the finding and name the reason:

```python
class Specialized(Base):
    def reset(self) -> None:
        self._state = None  # noqa: GR015 -- declared by Base
```

A class defined inside a test to stand in for a real object may not be worth
declaring. Suppress its findings, or exclude test files with
`per-file-ignores`.
