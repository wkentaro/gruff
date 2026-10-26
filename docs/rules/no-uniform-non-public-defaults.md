# no-uniform-non-public-defaults (GR002)

## What it does

Flags a default parameter when every call in its module passes it, or when no
call passes it. If some calls pass it and others omit it, the default stays.
Only module-level functions and methods whose own names start with `_` and do
not end with `_` are candidates.

## Why

A default that no call overrides acts as a fixed value. Put that value inside
the definition. A default that every call overrides serves no current caller.
Remove it so each caller must choose a value.

## Example

### No call passes the parameter

Move the fixed width into the function:

```diff
-def _resize_image(*, data: bytes, width: int = 512) -> bytes:
-    return resize(data, width=width)
+def _resize_image(*, data: bytes) -> bytes:
+    return resize(data, width=512)

 def make_thumbnail(data: bytes, /) -> bytes:
     return _resize_image(data=data)
```

### Every call passes the parameter

Remove the unused default and keep the parameter required:

```diff
-def _resize_image(*, data: bytes, width: int = 512) -> bytes:
+def _resize_image(*, data: bytes, width: int) -> bytes:
     return resize(data, width=width)

 def make_thumbnail(data: bytes, /) -> bytes:
     return _resize_image(data=data, width=256)
```

### Rule scope

A definition directly re-exported by a public ancestor package initializer is public under the [GR001 boundary](explicit-non-public-input-conventions.md) and is excluded, even if its original name starts with an underscore.

The rule reads only the definition's own module, so it skips a definition when it cannot see every use there:

- It has no calls in the module.
- A reference other than a direct call exists, such as a callback, a `functools.partial` argument, a string with its name, as in `getattr(self, "_load")`, or a method named bare in its class body, as in `__call__ = _load`.
- A call unpacks `*args` or `**kwargs`.
- A method is called through anything other than its class's name or the receiver of a method in that class, as in `service._load()`. The receiver is `self` in an undecorated method or `cls` in a classmethod, and only while the method never rebinds it.
- An instance method is called through `cls` or its class, as in `Service._load(service)`, or any method is called through a class that has a decorator, a base, or a metaclass.
- A method with a double-underscore name is referenced by its mangled name, as in `service._Service__load()`.
- A method input name starts with two underscores and does not end with two, so Python mangles it to `_Class__name`.
- Any decorator other than a single `staticmethod` or `classmethod` wraps it.
- An attribute has the function's name, as in `module._load()`.
- Another definition, parameter, import, assignment, `match` capture, or `except` name in the module binds the same name.
- The module has a wildcard import, rebinds `staticmethod` or `classmethod`, or defines `__getattribute__`.

It skips the whole module when a name built at runtime may reach a definition: `getattr`, `hasattr`, `setattr`, `delattr`, `attrgetter`, or `methodcaller` with a name that is not a string literal, any call to `eval`, `exec`, `globals`, `locals`, `vars`, or `__getattribute__`, any `__dict__`, `__globals__`, `__builtins__`, `f_globals`, `f_locals`, or `f_builtins` access, or any of these functions referenced or imported under another name.

The rule assumes `self` in a method is an instance of the method's class and `cls` in a classmethod is that class. It also assumes `self._load` finds the method the class defines, which a `__getattribute__` inherited from a base in another module can change. A method called unbound with an unrelated object, as in `Service.roll(Random())`, or such a base breaks these assumptions, and the rule may then attribute a call to the wrong method.

An ordinary name inside a non-public class, module, or package is outside the rule, because other modules may call it. Its default can hold a value that several modules share.

## When to suppress

Suppress the rule when the default is a policy that a future call should inherit, even though no current call overrides it:

```python
def _fetch(*, url: str, timeout: float = 30.0) -> bytes:  # noqa: GR002 -- service timeout policy
    return fetch(url, timeout=timeout)
```
