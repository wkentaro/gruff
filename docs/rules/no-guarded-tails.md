# no-guarded-tails (GR009)

## What it does

Flags an `if` statement with neither an `elif` nor an `else` that is the last statement directly in a function body (`def` or `async def`, at any nesting depth, including a function nested in another function or in a class) or in a loop body (`for`, `async for`, or `while`), when its suite meets a size gate: the suite spans at least ten physical lines — the last statement's end line minus the first statement's start line, plus one, so interior comments and blank lines count — or it lexically contains an `if` statement at any depth, including inside a nested loop, `with`, or `try`, and inside a function or class defined in the suite. A docstring ahead of the `if` does not disqualify the body. Each qualifying `if` is its own finding, anchored on the `if` keyword and its condition.

Only a direct child of a function or loop body counts, so an `if` at the tail of a `with`, `try`, `except`, `finally`, `else`, or another `if` suite is outside the rule; of two directly nested trailing ifs only the outer one is flagged, and the inner one becomes a direct child, and a finding, once the outer one is inverted. A loop's `else` suite is not a loop body, and module and class bodies are outside the rule entirely, having nothing to return from or continue. So are an `if` that carries an `elif` or an `else`, an `if` that is not the last statement of its body, and a suite under ten lines whose only further branching is a ternary expression or a `match` statement. The rule has no autofix.

## Why

When the tail of a body is one large `if`, the condition that governs it scrolls off the top of the screen and every remaining line carries an extra level of indentation. Inverting it moves the condition to the exit — `if not ready: return` — and the work it guards reads at the level it belongs to, with each further guard stacking flat instead of deeper.

The inversion is always behavior-equivalent here. The condition still evaluates exactly once, walrus bindings included; a bare `return` is valid in a generator and an async generator; and a `continue` does not disturb a `for`/`else`, which runs unless the loop breaks.

## Example

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

## When to suppress

Invert the condition and let the rest of the body dedent. Where the guard would read worse than the nesting — a negation that needs De Morgan's law to state, or a trailing branch deliberately kept parallel with its sibling branches — keep the shape and say so with `# noqa: GR009 -- reason` on the `if` line:

```python
def route(request):
    log_request(request)
    if request.is_authenticated and not request.is_expired:  # noqa: GR009 -- the guard needs De Morgan over both conditions
        session = load_session(request)
        if session.is_stale:
            session.refresh()
        return render(session)
```

The else-after-return shapes belong to Ruff `RET505` through `RET508`, which unwrap the `else` that follows a terminating branch. This rule deliberately skips them: an `if` with an `elif` or an `else` is outside it, so the two never contest the same statement, and running both leaves one flattening the branch that already returns and the other flattening the branch that never did.
