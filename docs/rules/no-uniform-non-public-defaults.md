# no-uniform-non-public-defaults (GR002)

## What it does

Flags a default on a module-level function or method whose own name starts with an underscore and does not end with one, when the calls in its module all treat that input the same way. The rule reports the input when no call passes it, or when every call passes it. A default that some calls pass and others omit is left alone.

The rule reads only the definition's own module, so it skips a definition when it cannot see every use there:

- It has no calls in the module.
- A reference other than a direct call exists, such as a callback or a `functools.partial` argument.
- A call unpacks `*args` or `**kwargs`.
- A method is called through anything other than its class's name or the receiver of a method in that class, as in `service._load()`. The receiver is `self` in an undecorated method or `cls` in a classmethod, and only while the method never rebinds it.
- An instance method is called through `cls` or its class, as in `Service._load(service)`.
- A method with a double-underscore name is referenced by its mangled name, as in `service._Service__load()`.
- A decorator other than `staticmethod` or `classmethod` wraps it.
- An attribute has the function's name, as in `module._load()`.
- Another definition, parameter, import, assignment, `match` capture, or `except` name in the module binds the same name.

An ordinary name inside a non-public class, module, or package is outside the rule, because other modules may call it. Its default can hold a value that several modules share.

## Why

A default that no call overrides is a fixed value that looks like a parameter. A reader has to check every call to learn that it never changes, so the value belongs inside the definition. A default that every call overrides only covers a case that never happens. Remove it so a new call has to choose a value. A default that some calls override keeps the common case short and is left alone.

## Example

```diff
-def _resize_image(*, data: bytes, width: int = 512) -> bytes:
-    return resize(data, width=width)
+def _resize_image(*, data: bytes) -> bytes:
+    return resize(data, width=512)

 def make_thumbnail(data: bytes, /) -> bytes:
     return _resize_image(data=data)
```

## When to suppress

Suppress the rule when the default is a policy that a future call should inherit, even though no current call overrides it:

```python
def _fetch(*, url: str, timeout: float = 30.0) -> bytes:  # noqa: GR002 -- service timeout policy
    return fetch(url, timeout=timeout)
```
