# no-non-public-imports (GR014)

## What it does

Flags each name in a `from module import name` statement when the name starts
with an underscore and does not end with one, and the module is part of the
same project. Dunder, sunder, and trailing-underscore names are not flagged.

A module is part of the project when the import is relative, or when the
absolute module resolves to a file under the importing file's import root. The
import root is the parent of the outermost directory with an `__init__.py` or
`__init__.pyi`. An import that does not resolve on disk is not flagged, so
`from os import _exit` and third-party imports pass.

## Why

A leading underscore tells the reader that a name is used only inside its own
module. An import from another module breaks that promise. A reader who
changes the name's behavior checks its own module for callers and misses the
import. Dropping the underscore makes the name honest about who uses it.

Ruff's PLC2701 skips imports inside the same top-level package, and SLF001
covers only attribute access such as `module._name`. Enable SLF001 with GR014
to cover both ways of reaching a non-public name.

## Example

Drop the underscore at the definition and every use:

```diff
 # labelme/_automation/_geometry.py
-def _round_bbox_to_int(bbox): ...
+def round_bbox_to_int(bbox): ...

 # labelme/_automation/_detect.py
-from labelme._automation._geometry import _round_bbox_to_int
+from labelme._automation._geometry import round_bbox_to_int
```

### Rule scope

Only the imported name counts. Underscores in the module path are not flagged,
because a `_`-prefixed module is private to its package, so
`from labelme._widgets._dialog import Dialog` is correct use. For the same
reason, a name that is a submodule or subpackage of the imported package, as
in `from pkg import _sub`, is not flagged.

Imports inside functions, under `if TYPE_CHECKING:`, and in `try` blocks are
checked like top-level imports. A package initializer is checked like any
other module. The rule does not look at what the name is bound to, so a
function, class, variable, or re-imported name all count.

Test files may import any non-public name. Gruff skips files named
`test_*.py`, `*_test.py`, or `conftest.py`. An absolute import resolves only
from the import root, so an import through a namespace package above the
outermost initializer is not flagged. Gruff has no autofix, and a fix renames
the name across modules.

## When to suppress

Rename the name when other modules need it.

Suppress when the module owner deliberately shares a name with one
collaborator, and renaming it would make it look like part of a public API:

```python
from ._registry import _register  # noqa: GR014 -- plugin hook shared with the loader only
```
