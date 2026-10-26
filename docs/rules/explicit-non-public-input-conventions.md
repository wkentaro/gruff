# explicit-non-public-input-conventions (GR001)

## What it does

Flags non-public function and method parameters that accept both positional and
keyword arguments. Use `/` for positional-only parameters and `*` for keyword-only
parameters. Implicit method receivers, `*args`, and `**kwargs` are excluded.

## Why

An explicit signature tells readers how to call a function without searching its
call sites. Python enforces that convention at runtime.

GR001 covers non-public definitions. [GR005](explicit-public-input-conventions.md)
applies the same policy to public definitions, so projects can adopt each scope
separately. A non-public name can still implement an external callback contract.

## Example

Keep image data positional and require the caller to name the width:

```diff
-def _resize_image(data: bytes, width: int) -> bytes:
+def _resize_image(data: bytes, /, *, width: int) -> bytes:
     return resize(data, width=width)

 def make_thumbnail(data: bytes, /) -> bytes:
     return _resize_image(data, width=512)
```

Parameters before `/` accept positional arguments only. Parameters after `*`
accept keyword arguments only. Inspect and update callers with the signature.

### Rule scope

A definition is non-public when its name starts with `_` and does not end with
`_`, or when an ordinary name sits inside a non-public class, module, or package.
This includes `_name` and `__name`, with Python's usual name mangling in classes.
For example, `_helpers.py` and `_impl/core.py` make ordinary definitions non-public.

Package scope stops at the import root, the last directory reached by walking
up through directories containing `__init__.py` or `__init__.pyi`. Names above
that root do not count. Trailing-underscore, sunder, and dunder definitions are
excluded, as are nested functions and methods of classes defined inside functions.

Direct re-exports from public ancestor package initializers override this
boundary. For example, `from ._resize import resize` in a public `__init__.py`
makes `resize` public even though its defining module is non-public. A literal
list or tuple assigned to `__all__` limits exports to its entries. Without a
manifest, imported names that do not start with `_` are public.

Relative and absolute imports, including namespace-qualified imports, respect
package-over-module precedence. Re-export discovery crosses namespace directories
without initializers. Absolute imports are resolved from ancestors above the
outermost initialized package, never from inside that package.
Import aliases are respected. The defining name
must still refer to one function or class declaration, allowing overloads.
Reassigning that name prevents promotion. Ordinary methods of a re-exported class
are public too; private method names and nested private classes stay non-public.

The analysis reads source without importing Python. It resolves only
unconditional, direct `from` imports from the defining module into an ancestor
package initializer. Shadowed or conditional imports, forwarding modules,
wildcard imports, dynamic namespace access, and dynamic or mutated manifests
remain unresolved. Assertions and comparisons may read a literal manifest;
other reads are unresolved because they can expose it to mutation. Unresolved
exports retain the name-and-scope classification above.

## When to suppress

Suppress a definition when an external contract requires both call styles.
Otherwise, choose a convention and migrate its callers.

A type check alone cannot prove callback compatibility. Frameworks may dispatch
callbacks through `Any` or a callable type that omits their parameters. Test the
actual registration and dispatch path, and check that the callback ran and
produced the expected result without errors.

This standard-library scheduler dispatches the same callback both ways:

```python
import sched

events: list[str] = []


def _record_event(event: str) -> None:  # noqa: GR001 -- contract accepts both call styles
    events.append(event)


scheduler = sched.scheduler()
scheduler.enter(delay=0, priority=0, action=_record_event, argument=("positional",))
scheduler.enter(delay=0, priority=0, action=_record_event, kwargs={"event": "keyword"})
scheduler.run()
assert events == ["positional", "keyword"]
```

Adding `*` before `event` breaks the positional dispatch. Adding `/` after it
breaks the keyword dispatch. A direct call tailored to either new signature
would miss the failure. If you control the registrations, change them together
with the signature; otherwise, retain the suppression.

Prefer inline suppressions with a reason. Before adding a per-file ignore, run
the rule without it and audit every GR001 finding in every matched file, including
existing inline exceptions. Use a per-file ignore only when all findings share
the same intentional contract exception. Fix unrelated findings and keep inline
suppressions for exceptions with different reasons. A per-file ignore also hides
future findings that may have nothing to do with the audited exception.
