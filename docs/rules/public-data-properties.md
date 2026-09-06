# public-data-properties (GR012)

## What it does

Flags each direct store to a public attribute of an instance method's first positional parameter, unless that class declares a setter for the same name. The finding points at the attribute name, with the diagnostic `Public instance attribute <name> requires a property setter for writes; use underscore-prefixed storage for internal state`. Public means the attribute name does not start with `_`; a class name starting with `_` does not exempt its fields.

The rule covers ordinary, annotated (including annotation-only), augmented, chained, and unpacking assignments, as well as `for` and `with` targets, anywhere in a method's control flow, including outside `__init__`. A receiver may have any name and may be positional-only. Reads, method calls, item mutation, and deletion are not stores of an attribute and are outside this rule.

A same-class `@name.setter` on a method named `name` permits writes; `@Base.name.setter` also explicitly declares a local setter. `@property` alone permits annotation-only declarations but does not permit writes. Bare `property`, `staticmethod`, `classmethod`, and their `builtins.` spellings are recognized syntactically. Static methods, class methods (including the implicit `__init_subclass__` and `__class_getitem__` hooks), and `__new__` are excluded. Methods must be direct statements of their class body; nested functions (including lambdas) are not followed, but nested classes are checked independently. Import aliases, decorator rebinding, receiver rebinding, and runtime property replacement are not resolved.

The following boundaries are deliberate:

- **Inherited properties:** bases and the method resolution order are not resolved, even for a base in the same file. A local explicit setter is recognized; suppress writes dispatched to an inherited setter otherwise. Subclasses are not exempt as a whole.
- **Descriptors:** declarations such as `@cached_property` or `field = Descriptor()` are outside the store shape. Direct writes to those public names still need suppression unless a recognized setter exists. No descriptor behavior is guessed.
- **Class constants and `ClassVar`:** all class-body fields, including annotation-only declarations and default values, are outside this instance-store rule. A store through the receiver remains in scope even if the class body declares the name as `ClassVar` or `Final`.
- **Dataclasses, attrs, and models:** generated fields and constructors are outside the source shape. Explicit receiver stores in user-written methods are checked normally; suppress stores required by the schema or framework.
- **Slots:** strings in `__slots__` are not stores. Direct stores to public slot names are flagged; underscore-prefixed slots are accepted.
- **Frameworks and protocols:** public spellings mandated externally use a suppression with the contract's reason. Classes with a base or decorator receive no blanket exemption.
- **Other objects and dynamic writes:** aliases of the receiver, `setattr`, `__dict__` updates, and stores through another object are outside this rule. Gruff does not infer types or validate callers' writes to read-only properties; use a type checker for that contract.

The rule is opt-in, supports normal selection and suppression, and has no autofix: renaming storage or introducing a property can change an external contract.

## Why

A raw public field makes storage an implicit data interface. Internal state only needs an underscore-prefixed name; intentional public access deserves a property, and intentional public writes deserve a setter. Public methods remain methods.

Pair GR012 with Ruff's [private-member-access (SLF001)](https://docs.astral.sh/ruff/rules/private-member-access/), derived from flake8-self. It flags external backing-field access such as `logger._error_message`, including when a private class and its caller share a module. Gruff deliberately leaves this existing rule to Ruff. SLF001 has its own documented exceptions and is a syntactic companion, not complete access control. Pylint also provides [protected-access (W0212)](https://pylint.pycqa.org/en/latest/user_guide/messages/warning/protected-access.html). Neither requires public storage to become a property; Ruff's `RUF012` addresses mutable class defaults and `PLR0206` addresses property parameters instead.

## Example

```python
class _Result:
    def __init__(self) -> None:
        self.error_message: str | None = None
```

For state used only by the class, rename it to `_error_message`. If callers need to read it, expose that state explicitly:

```python
class _Result:
    def __init__(self) -> None:
        self._error_message: str | None = None

    @property
    def error_message(self) -> str | None:
        return self._error_message

    @error_message.setter
    def error_message(self, value: str | None) -> None:
        self._error_message = value

    def clear(self) -> None:
        self.error_message = None


result = _Result()
print(result.error_message)
```

Omit the setter and `clear` method if public writes are not needed. External callers must use `result.error_message`; replacing their access with `result._error_message` violates the boundary and is flagged by SLF001.

## When to suppress

Keep an externally imposed public field or a write dispatched to an inherited property or custom descriptor, and name the reason on the attribute's line:

```python
class DownloadError(Exception):
    def __init__(self, message: str) -> None:
        self.message = message  # noqa: GR012 -- downstream clients require this field
```

For a multiline target, put the suppression on the line containing the attribute name. A per-file ignore can cover files consisting entirely of framework data models. Suppression records a deliberate contract; blindly renaming a field can break its callers.
