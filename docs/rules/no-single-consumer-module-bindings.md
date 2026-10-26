# no-single-consumer-module-bindings (GR011)

## What it does

Flags a non-public module binding read by exactly one function or method. Move
the value into that consumer when it does not need to persist between calls.
Gruff has no autofix.

## Why

A value used by one function is easier to find inside that function. Moving it
also removes a name from the module namespace. Keep values at module scope when
they must be shared or evaluated only once.

## Example

Move the lookup table into the function that reads it. The local constant keeps
its uppercase spelling and `Final` annotation under GR004; it can drop the
leading underscore:

```diff
 from typing import Final

-_FORMATS: Final = {"rgb": "RGB", "rgba": "RGBA"}
-
-
 def normalize_mode(mode: str, /) -> str:
+    FORMATS: Final = {"rgb": "RGB", "rgba": "RGBA"}
-    return _FORMATS[mode]
+    return FORMATS[mode]
```

The function now builds the table on each call. Check the evaluation cost and
whether it reads values that can change before moving a binding.

### Rule scope

Candidates are direct module-body assignments of `_name = value` or
`_name: T = value`. Names must start with one underscore and not end with one.
Values containing a call at any depth, and empty list or dict displays, are
excluded because they may hold cached work or state across calls.

A consumer is a direct module-level `def` or `async def`, or a method directly
inside a module-level class, including static methods, class methods, and
properties. Reads inside its nested functions, lambdas, and comprehensions count
as that consumer's reads.

The binding stays at module scope when:

- Any other binding of the name exists anywhere in the module, including a local,
  parameter, import, assignment target, `del`, `global`, or `nonlocal` declaration.
- A read occurs outside a consumer body, including in its decorators, defaults,
  or annotations, or reads come from multiple consumers. A string annotation is
  a string literal, not a read.
- The name appears in `__all__`, or nobody reads it.
- An attribute or subscript store or delete leads back to the name, as in
  `_state.rows[key] = value`. That is shared state.

Public and double-underscore names, chained and unpacking assignments,
annotation-only declarations, class attributes, and bindings or consumers inside
module-level control flow are excluded. Reading `globals`, `vars`, `eval`, or
`exec` by name makes Gruff skip the whole module.

For `_A = 1` and `_B = _A + 1` used only by `f`, Gruff first flags `_B`. After
moving it into `f`, run Gruff again; `_A` may then qualify.

## When to suppress

Keep module scope when values need to stay together, another module imports
the name, or the value holds state between calls. Gruff cannot identify every
mutation: method calls such as `append` and stores into a method's return value
can look like reads.

Put the reason on the assignment's first line:

```python
_LOGGER_LEVELS: Final = ("debug", "info", "warning", "error", "critical")  # noqa: GR011 -- tests/unit/__main___test.py parametrizes over it
```

Gruff analyzes one module at a time and cannot see that import. Ruff's
`PLC2701`, `import-private-name`, checks the importing side. Suppressions record
the intentional export when both rules are enabled.
