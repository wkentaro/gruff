# explicit-instance-data (GR012)

## What it does

Flags each direct store to a public attribute of an instance method's first positional parameter, unless that class explicitly declares the instance field or a setter for the same name. An annotation directly in the same class body declares the field, with or without a default. This applies equally to ordinary classes, dataclasses, attrs classes, and Pydantic models; no framework recognition is required. An annotation inside a method, such as `self.value: int = 1`, does not declare a class interface.

The finding points at the attribute name, with the diagnostic ``Instance data field `<name>` is implicit; use private storage for internal state, or declare instance data or expose a property for public access.``. Public means the attribute name does not start with `_`; a class name starting with `_` does not exempt its fields.

The rule covers ordinary, annotated (including annotation-only), augmented, chained, and unpacking assignments, as well as `for` and `with` targets, anywhere in a method's control flow, including outside `__init__`. A receiver may have any name and may be positional-only. Reads, method calls, item mutation, and deletion are not stores of an attribute and are outside this rule.

A same-class `@name.setter` on a method named `name` permits writes; `@Base.name.setter` also explicitly declares a local setter. `@property` alone permits annotation-only receiver declarations but does not permit writes, even when the name also has a class-body annotation. Such writes receive the diagnostic ``Write to getter-only property `<name>`; use private storage, or declare a setter for intentional public writes.``. Bare `property`, `staticmethod`, `classmethod`, and their `builtins.` spellings are recognized syntactically. Static methods, class methods (including the implicit `__init_subclass__` and `__class_getitem__` hooks), and `__new__` are excluded. Methods must be direct statements of their class body; nested functions (including lambdas) are not followed, but nested classes are checked independently. Decorator aliases, decorator rebinding, receiver rebinding, and runtime property replacement are not resolved.

Annotations whose outer name is `ClassVar` or `InitVar` do not declare instance fields. Bare, qualified, subscripted, and quoted forms are recognized, as are direct import aliases from `typing` or `typing_extensions` for `ClassVar` and `dataclasses` for `InitVar`. Alias tracking is syntactic: imports inside control-flow blocks belong to their enclosing scope. Module and function aliases are visible in nested scopes, while class-local aliases apply only to that class’s annotations, not to nested classes or methods. Rebinding, re-exports, and assignment-based type aliases are not resolved. Unknown annotations, including unparseable quoted annotations, still declare a name; Gruff does not validate annotation types or infer runtime field semantics.

The following boundaries are deliberate:

- **Inherited annotations and properties:** bases and the method resolution order are not resolved, even for a base in the same file. A local explicit setter is recognized; suppress writes covered by inherited declarations otherwise. Subclasses are not exempt as a whole.
- **Descriptors:** declarations such as `@cached_property` or `field = Descriptor()` are outside the store shape. Direct writes to those public names need a class-body annotation, recognized setter, or suppression. No descriptor behavior is guessed.
- **Class constants and annotations:** class-body assignments are outside this instance-store rule. An unannotated default does not declare instance data, nor does a recognizable `ClassVar` or `InitVar` annotation. Ordinary annotations and `Final` declare the name; frozen-class writes and `Final` reassignment enforcement belong to other tools. Annotations nested under class-body conditionals are not direct declarations and are not recognized.
- **Dataclasses, attrs, and models:** generated fields and constructors are outside the source shape. Explicit receiver stores in user-written methods are accepted for annotated fields, including in `__init__` and `__post_init__`. Unannotated `attrs.field()` assignments are not recognized; decorators and bases provide no blanket exemption for undeclared names.
- **Slots:** strings in `__slots__` are not stores. Direct stores to unannotated public slot names are flagged; underscore-prefixed slots are accepted.
- **Frameworks and protocols:** public spellings mandated externally use a suppression with the contract's reason. Classes with a base or decorator receive no blanket exemption.
- **Other objects and dynamic writes:** aliases of the receiver, `setattr`, `__dict__` updates, and stores through another object are outside this rule. Gruff does not infer types or validate callers' writes to read-only properties; use a type checker for that contract.

The rule is opt-in, supports normal selection and suppression, and has no autofix: renaming storage or introducing a property can change an external contract.

## Why

A public field introduced only inside a method makes storage an implicit data interface. Class-body annotations make that interface explicit without changing storage behavior. For a data-carrier class, prefer a dataclass: declared fields and a generated constructor make its data explicit without adding properties for each field. Small predicates do not prevent a class from being a data carrier. When behavior, validation, or read/write control matters, use underscore-prefixed backing storage and properties; a setter is needed only for intentional public writes. State used only inside the class needs an underscore-prefixed name without a property. Public methods remain methods.

This policy applies independently of class or module naming: consumers determine the effective interface, and a non-public helper can later become part of one. The rule does not classify classes by their number of fields or amount of behavior.

Pair GR012 with Ruff's [private-member-access (SLF001)](https://docs.astral.sh/ruff/rules/private-member-access/), derived from flake8-self. It flags external backing-field access such as `logger._error_message`, including when a private class and its caller share a module. Gruff deliberately leaves this existing rule to Ruff. SLF001 has its own documented exceptions and is a syntactic companion, not complete access control. Pylint also provides [protected-access (W0212)](https://pylint.pycqa.org/en/latest/user_guide/messages/warning/protected-access.html). Neither requires public storage to become a property; Ruff's `RUF012` addresses mutable class defaults and `PLR0206` addresses property parameters instead.

## Example

```python
class _Result:
    def __init__(self) -> None:
        self.error_message: str | None = None
```

For a data carrier, declare the field and let the dataclass generate the constructor:

```python
from dataclasses import dataclass


@dataclass
class _Result:
    error_message: str | None = None
```

A class-body annotation also works with a handwritten constructor or update method, without any decorator:

```python
class _Result:
    error_message: str | None

    def __init__(self) -> None:
        self.error_message = None

    def clear(self) -> None:
        self.error_message = None
```

The same declaration permits explicit writes in dataclass and model methods, including `__post_init__`. Undeclared names remain checked.

For state used only by the class, use `_error_message`. When read/write control or behavior matters, expose private backing storage through a property:

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

For a public field you own, prefer a class-body annotation. When the declaration is inherited or a custom descriptor requires an otherwise unrecognized write, suppress the write and name the reason on the attribute's line:

```python
class SpecializedResult(ExternalResult):
    def clear(self) -> None:
        self.value = None  # noqa: GR012 -- declared by ExternalResult
```

For a multiline target, put the suppression on the line containing the attribute name. Exclude generated or vendored output in the consuming project, while keeping project-owned generators subject to the rule. Suppression records a deliberate contract; blindly renaming a field can break its callers.
