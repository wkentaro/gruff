# no-single-consumer-module-bindings (GR011)

## What it does

Flags a non-public module binding whose every read sits inside exactly one definition. The binding is a direct child of the module body that assigns a plain name — `_name = value` or `_name: T = value` — where the name starts with one underscore and does not end with one, and the value is neither a call expression at any depth nor an empty list or dict display. The one definition is a consumer: a `def` or `async def` that is a direct child of the module body, or a direct child of a module-level class body, `staticmethod`, `classmethod`, and `property` methods included. A read anywhere lexically inside the consumer's body counts as that consumer's read, including from a nested function, a lambda, or a comprehension. Findings are anchored on the assigned name; the message names the consumer, as `resolve` for a function or `Dialog.apply` for a method. The rule has no autofix.

A binding stays where it is when any of these hold: the name is bound anywhere else in the module, whether by a second module-level assignment, an augmented assignment, a walrus, `del`, a `global` or `nonlocal` declaration, an import, a `def` or `class`, a loop, `with`, `except`, or `match` target, a parameter, or a same-named local in any scope; a read sits outside a consumer body, in a module-level statement, a class body, or in a consumer's own decorators, defaults, or annotations, which all evaluate before a moved binding would exist; the reads come from two or more consumers; the name appears as a string in `__all__`; or there is no read at all, since a dead binding is a different policy. A string annotation naming the binding is a constant, not a read. A read that mutates the binding in place — a subscript or attribute store or delete whose chain leads back to the name, such as `_cache[key] = value` or `_state.rows[key] = value` — keeps it at module scope, since it is state shared across calls rather than a value the consumer reads. Public names, double-underscore names, which a class body mangles so a method never reads the module binding, chained and unpacking assignments, annotation-only declarations, class attributes, and any binding nested in a module-level `if`, `try`, `with`, or loop are not candidates, and a definition nested in one of those is not a consumer. A value that calls anything is excluded because a called value may be memoising work its consumer would otherwise repeat on every run, and an empty list or dict is excluded because it is an accumulator the consumer fills across calls. The rule skips a module entirely when it reads `globals`, `vars`, `eval`, or `exec` by name, since the namespace is then reachable by means a lexical walk cannot see.

With `_A = 1` and `_B = _A + 1` read only by `f`, the first run flags `_B` alone, because `_A` is read at module level by `_B`'s value. Once `_B` moves into `f`, `_A` becomes a single-consumer binding and the next run flags it.

## Why

A binding read by one definition serves nobody else, yet at module scope it widens every reader's search to the whole module, becomes state the module carries for its lifetime, and can be reached from any other module by import. Placing it inside the definition that reads it makes the value visible where it is used, drops the reader's jump, and leaves the module namespace to the names that are shared. The move is behavior-preserving for a value the consumer only reads: a literal, a container of literals that is looked up but never stored into, an attribute chain, or an arithmetic expression means the same wherever it is written.

The moved binding keeps its spelling under GR004: an uppercase name stays uppercase and keeps its `Final` annotation inside the function. The leading underscore may go, because a function-local name has no public or non-public distinction.

## Example

```diff
-_QIMAGE_FORMATS: Final = {
-    "RGB": QImage.Format.Format_RGB888,
-    "RGBA": QImage.Format.Format_RGBA8888,
-}
-
-
 class BrightnessContrastDialog(QtWidgets.QDialog):
     def apply(self) -> None:
+        QIMAGE_FORMATS: Final = {
+            "RGB": QImage.Format.Format_RGB888,
+            "RGBA": QImage.Format.Format_RGBA8888,
+        }
         image = self.adjust()
-        qimage = QImage(image.tobytes(), *image.size, _QIMAGE_FORMATS[image.mode])
+        qimage = QImage(image.tobytes(), *image.size, QIMAGE_FORMATS[image.mode])
```

## When to suppress

Move the binding into the definition that reads it. Where module scope is the point — a table of related constants kept together because a sibling has a second consumer, a non-empty container the consumer mutates through a method call such as `append`, or stores into the result of one, which the rule cannot tell from a read, or a name another module imports on purpose, such as a test that parametrizes over it — keep the binding and say so with `# noqa: GR011 -- reason` on the assignment's first line:

```python
_LOGGER_LEVELS: Final = ("debug", "info", "warning", "error", "critical")  # noqa: GR011 -- tests/unit/__main___test.py parametrizes over it
```

The rule reads one module at a time, so it cannot see that import. Ruff `PLC2701` (`import-private-name`) flags the importing side of a non-public name; with both enabled, a test-only export is reported where it is written and where it is used, and the suppression above records the choice to keep it.
