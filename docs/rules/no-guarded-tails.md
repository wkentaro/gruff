# no-guarded-tails (GR009)

## What it does

Flags a trailing `if` directly inside a function or loop body when it has no
`elif` or `else` and its body spans at least ten physical lines or contains
another `if`. Replace the outer condition with a `return` or `continue` guard.

## Why

An early guard removes one level of indentation from the remaining work. Readers
can check the exit condition once, then follow the work without carrying a
condition from the top of a long block.

## Example

Return when no record exists, then update the user without the outer nesting:

```diff
 def sync_user(user):
     record = fetch_record(user.id)
-    if record is not None:
-        user.name = record.name
-        user.email = record.email
-        if record.is_admin:
-            user.grant_admin()
-        user.save()
+    if record is None:
+        return
+
+    user.name = record.name
+    user.email = record.email
+    if record.is_admin:
+        user.grant_admin()
+    user.save()
```

### Rule scope

Functions include `def` and `async def` at any nesting depth. Loops include `for`,
`async for`, and `while`. A preceding docstring is allowed. The ten-line count
runs from the first statement's start to the last statement's end, including
interior comments and blank lines. A nested `if` anywhere in the suite counts,
even inside a nested function or class.

Only the last direct statement of a function or loop body qualifies. Module and
class bodies, loop `else` suites, and the tails of `with`, `try`, `except`,
`finally`, `else`, or other `if` suites are excluded. Ternary expressions and
`match` statements do not meet the nested-`if` gate.

For directly nested trailing `if` statements, fix the outer finding and run Gruff
again; the inner `if` may then qualify. Gruff has no autofix.

## When to suppress

Keep the nesting if the guard would be harder to read, for example because it
needs De Morgan's law over several conditions. Put the reason on the `if` line:

```python
def route(request):
    log_request(request)
    if request.is_authenticated and not request.is_expired:  # noqa: GR009 -- the guard needs De Morgan over both conditions
        session = load_session(request)
        if session.is_stale:
            session.refresh()
        return render(session)
```

Use `return` in a function and `continue` in a loop. The condition still evaluates
once, including walrus bindings. A bare `return` also works in generators and
async generators. `continue` preserves a loop's `else` behavior, which depends
on whether the loop breaks.

Ruff's `RET505` through `RET508` handle `else` after a terminating branch. GR009
skips statements with `elif` or `else`, so the rules cover different shapes.
