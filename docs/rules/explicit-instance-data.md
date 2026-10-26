# explicit-instance-data (GR012)

## What it does

Flags public instance attributes assigned in a method without a field declaration
or setter in the same class. Declare public fields in the class body, or use
underscore-prefixed names for internal state.

An annotation on `self.value` inside a method does not declare a field. The rule
also applies to classes whose names start with `_`.

## Why

A field created only inside a method leaves readers to search for the class's
data interface. Class-body annotations list its public fields in one place.
Private storage keeps internal state out of that interface. Properties let callers
read a value while the class controls how it changes.

## Example

### Declare a public field

Move the annotation to the class body. Callers can still read and write
`result.error_message`.

```diff
 class Result:
+    error_message: str | None
+
     def __init__(self) -> None:
-        self.error_message: str | None = None
+        self.error_message = None
```

For a data carrier, a dataclass also generates the constructor:

```diff
+from dataclasses import dataclass
+
+
+@dataclass
 class Result:
-    def __init__(self) -> None:
-        self.error_message: str | None = None
+    error_message: str | None = None
```

A dataclass changes more than the field declaration, including equality and
`repr`. Use the first option if you only need to declare the field.

### Keep internal state private

If callers do not need the field, prefix its name with `_`. Update its uses
inside the class too.

```diff
 class Result:
     def __init__(self) -> None:
-        self.error_message: str | None = None
+        self._error_message: str | None = None

     def failed(self) -> bool:
-        return self.error_message is not None
+        return self._error_message is not None
```

### Expose a read-only property

If callers need to read the value but the class should control writes, expose
private storage through a property:

```diff
 class Result:
     def __init__(self) -> None:
-        self.error_message: str | None = None
+        self._error_message: str | None = None
+
+    @property
+    def error_message(self) -> str | None:
+        return self._error_message

     def clear(self) -> None:
-        self.error_message = None
+        self._error_message = None
```

Callers can still read `result.error_message`. Assigning to it now raises
`AttributeError`. Add an `@error_message.setter` only if public writes are part
of the interface. A getter-only property does not permit writes through
`self.error_message`, even with a class-body annotation.

Pair GR012 with Ruff's
[private-member-access (SLF001)](https://docs.astral.sh/ruff/rules/private-member-access/)
to flag callers that access `result._error_message`. SLF001 has exceptions and
does not provide runtime access control.

### Rule scope

GR012 is opt-in and has no autofix. Renaming fields or adding properties can
break callers.

- It checks direct attribute targets on an instance method's first positional
  parameter, whatever its name, including positional-only parameters. Assignments,
  annotations without values, augmented assignments, chained and unpacking
  assignments, and `for` and `with` targets count, in any control flow and in
  methods beyond `__init__`.
- Fields need an annotation directly in the same class body, with or without a
  default. This works for ordinary classes, dataclasses, attrs classes, and
  Pydantic models, including explicit writes in `__post_init__`. Unannotated
  defaults, `attrs.field()` calls, `__slots__` entries, and declarations inside
  class-body conditionals do not declare fields.
- `ClassVar` and `InitVar` annotations do not declare instance fields. Gruff
  recognizes bare, qualified, subscripted, and quoted forms, plus direct import
  aliases for `typing.ClassVar`, `typing_extensions.ClassVar`, and
  `dataclasses.InitVar`. Imports follow
  lexical scope, including imports inside control flow. Class-local aliases apply
  only to that class's annotations. Rebinding, re-exports, and assignment-based
  aliases are not resolved. Other annotations, including `Final` and unparseable
  strings, declare fields. Gruff does not validate types or enforce frozen fields.
- A same-class `@name.setter` on a method named `name` permits writes, as does
  `@Base.name.setter`. A getter-only property permits annotation-only targets,
  but no writes. Gruff recognizes bare and `builtins.` spellings of `property`,
  `staticmethod`, and `classmethod`, without resolving aliases or rebinding.
- Static methods, class methods, `__new__`, `__init_subclass__`, and
  `__class_getitem__` are excluded. Only methods directly in a class body are
  checked. Nested functions and lambdas are skipped; nested classes are checked
  independently.
- Reads, calls, item mutation, deletion, and class-body assignments are outside
  this rule. So are receiver aliases, `setattr`, `__dict__` updates, and writes
  through other objects. Receiver rebinding and runtime property replacement are
  not resolved. Use a type checker to check callers' writes to read-only properties.

## When to suppress

Gruff does not resolve inherited declarations, even in the same file, or infer
custom descriptor behavior. If a base class or an external contract already
defines the field, suppress the finding and name the reason:

```python
class SpecializedResult(ExternalResult):
    def clear(self) -> None:
        self.value = None  # noqa: GR012 -- declared by ExternalResult
```

For fields you own, prefer a class-body annotation. Custom descriptors and
`cached_property` receive no special treatment; direct writes need an annotation,
a recognized setter, or a suppression. Bases and decorators do not exempt a class.

Put the suppression on the line containing the attribute name, including in a
multiline target. Exclude generated or vendored output in the consuming project;
keep project-owned generators subject to the rule.
