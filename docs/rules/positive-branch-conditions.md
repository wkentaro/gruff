# positive-branch-conditions (GR010)

## What it does

Flags an `if` with a plain `else` and no `elif` when its condition is a
`not` expression, or a single `is not`, `!=`, or `not in` comparison. Swap the branches
and state the condition positively.

## Why

A positive condition lets readers match the test directly to the first branch.
Both branches already exist, so swapping them removes the negation without
adding another branch.

## Example

Handle the missing record first and move the update into `else`:

```diff
-if record is not None:
-    user.name = record.name
-    user.save()
-else:
-    log_missing(user)
+if record is None:
+    log_missing(user)
+else:
+    user.name = record.name
+    user.save()
```

### Rule scope

The rule checks `if` statements at every nesting depth in modules, classes,
functions, and loops. Only the condition's outermost operation counts:
`not (a == b)` qualifies, but `a and not b` does not. Chained comparisons such
as `a is not b is not c`, ternary expressions, `if` statements without `else`,
and `elif` chains are excluded. Gruff has no autofix.

Inverting `not`, `is not`, and `not in` preserves the condition's behavior. For
`!=`, replacing it with `==` assumes `__ne__` complements `__eq__`.

## When to suppress

Keep the original branch order when the positive condition is harder to read or
when the order serves a deliberate purpose. Also suppress comparisons whose
`__ne__` and `__eq__` are not complements. Put the reason on the `if` line:

```python
if version != EXPECTED:  # noqa: GR010 -- Version.__ne__ compares ranges, not equality
    reject(version)
else:
    accept(version)
```

Ruff's `SIM212` covers the ternary `b if not a else a`. GR010 covers statements;
other negated ternaries are outside both rules.
