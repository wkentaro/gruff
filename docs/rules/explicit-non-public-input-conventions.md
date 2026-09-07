# explicit-non-public-input-conventions (GR001)

## What it does

Flags each fixed caller-supplied input to a non-public module-level function or method that is positional-or-keyword. Positional-only (`/`) and keyword-only (`*`) inputs declare an explicit calling convention and are accepted; implicit method receivers and variadic parameters are excluded.

A non-public definition starts with an underscore and does not end with one. This includes `_name` and `__name` spellings; double-leading names are name-mangled in class scope. Ordinary, trailing-underscore, sunder, and dunder definitions are outside the rule. Nested functions and methods of a class defined inside a function are also outside the rule.

## Why

A positional-or-keyword input leaves its calling convention to each call site, so a reader has to collect the callers before knowing how the input is passed. Positional-only and keyword-only declarations make the convention local, deterministic, and enforced at runtime.

Non-public definitions let projects adopt the policy separately from public definitions, but underscore naming does not prove that the project controls every caller. A non-public definition can still implement an externally imposed callback or protocol contract. `explicit-public-input-conventions` (GR005) carries the same policy to the complementary public bucket, and the two rules partition every definition without inferring whether an interface is documented or exported.

## Example

```diff
-def _resize_image(data: bytes, width: int) -> bytes:
+def _resize_image(data: bytes, /, *, width: int) -> bytes:
     return resize(data, width=width)

 def make_thumbnail(data: bytes, /) -> bytes:
     return _resize_image(data, width=512)
```

## When to suppress

Fix the finding by default when the project controls the calling contract: inspect callers, choose positional-only for a value whose role the name already carries and keyword-only for the rest, then update callers together with the signature. For framework callbacks and protocol implementations, first establish the external contract regardless of the definition's name.

A passing type check does not prove callback compatibility. A framework may store a callback behind `Any` or a callable type that does not describe its parameters, then invoke it dynamically with arguments the type checker cannot match to its signature.

After changing `/` or `*` placement, run the affected framework lifecycle or a focused smoke test that reaches the callback through the framework's actual registration and dispatch path. Check that the callback ran and produced its expected effect without callback errors. Calling it directly with arguments tailored to the new signature is insufficient.

For example, this runnable standard-library scheduler example models a contract requiring both call styles:

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

Changing the callback to `def _record_event(*, event: str) -> None` makes `scheduler.run()` raise `TypeError` on the positional dispatch, even though a direct `_record_event(event="positional")` call succeeds. Changing it to `def _record_event(event: str, /) -> None` breaks the keyword dispatch instead. If those call styles are imposed by an external contract, retain the original signature and suppress the finding; if the project controls the registrations, migrate them with the signature.

Suppress only when the definition is bound to an external contract that must keep accepting both positional and keyword calls:

```python
def _handler(request: Request, context: Context) -> Response:  # noqa: GR001 -- contract accepts both call styles
    return respond(request, context)
```

Prefer an inline suppression because it keeps the exception next to its reason. For a file made entirely of such contracts, use a per-file ignore instead.
