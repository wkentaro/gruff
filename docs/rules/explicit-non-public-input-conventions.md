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
