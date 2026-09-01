# positive-branch-conditions (GR010)

## What it does

Flags an `if` statement that carries a plain `else` and no `elif` — exactly one trailing clause, and that clause has no test — when the outermost operation of its condition is a negation: a unary `not`, or a single-comparator comparison whose operator is `is not`, `!=`, or `not in`. The swap then states the condition positively: `if x is None: fallback else: main` instead of `if x is not None: main else: fallback`. The rule applies to `if` statements anywhere — module, class, function, or loop body, at any nesting depth — and each qualifying statement is its own finding, including one written inside the branches of another finding. Findings are anchored on the `if` keyword and its condition. The rule has no autofix.

Only the outermost node of the condition decides, so a negation nested inside `and`, `or`, or any operand is outside the rule: `if x and not y:` is not flagged, while `if not (a == b):` is, the unary `not` being outermost. A chained comparison of two or more comparators, such as `a is not b is not c`, is not a negated condition; there is no single operator to invert. An `if` without an `else` is outside the rule — there is no second branch to swap into, and that shape is what GR009 asks for — and so is any `if` carrying an `elif`, since the swap is not mechanical across a chain. Ternary expressions are outside the rule entirely.

## Why

A negation-free condition is verified faster: the reader evaluates the test and takes the branch, instead of evaluating the test, inverting it, and then taking the branch. With a plain `else` both branches already exist, so the repair costs nothing structural — swap the two suites, drop the negation, and the statement says the same thing with one fewer operation for a reader to carry.

The swap is behavior-equivalent for `not` (one truthiness evaluation either way), for `is not` (identity, which has no hooks to disagree with), and for `not in` (defined as the negation of `in`). For `!=` it assumes `__ne__` complements `__eq__`, which is the contract every well-behaved class keeps; a class that breaks it is the suppression case.

## Example

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

## When to suppress

Swap the branches and drop the negation. Where the positive form is worse — a class whose `__ne__` deliberately does not complement `__eq__`, a negated condition whose positive form has no direct spelling or reads as a double negative, or a branch order kept on purpose, such as the main path first when the negated branch is long — keep the shape and say so with `# noqa: GR010 -- reason` on the `if` line:

```python
if version != EXPECTED:  # noqa: GR010 -- Version.__ne__ compares ranges, not equality
    reject(version)
else:
    accept(version)
```

The twisted-arms ternary — `b if not a else a` — belongs to Ruff `SIM212`, which unwinds the negation in that expression shape; other negated ternaries are flagged by neither tool, since both arms sit on one visible line. This rule covers statements only, so the two never contest the same code.
