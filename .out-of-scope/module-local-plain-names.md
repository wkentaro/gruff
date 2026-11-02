# Module-local plain names

A rule that flags an ordinary name in a non-public module because only its own module reads it, and asks for an underscore, is out of scope. An example is `AiAssistModelOption` in `labelme/_ai_models.py` when no other module imports it.

GR014 `no-non-public-imports` covers the opposite direction, a `_name` that another module imports. That rule only needs the import statement. This one would need to know every reader in the project.

## Why this is out of scope

The rule contradicts ADR 0016. That ADR says a project marks privacy once, where it starts, instead of repeating it on every name inside a private module. This rule would require repeating the underscore on every name that only its own module reads, which splits "private to the package" from "private to the module" again.

A trial in #109 counted candidates: ordinary module-level `def`, `class`, and binding names in non-public modules, not listed in `__all__`, not re-exported by an ancestor package, and not imported or read as an attribute by another non-test module.

| Repository | Read only by own module | Read only by tests | Name also appears as text elsewhere |
| --- | --- | --- | --- |
| labelme at 674f67b | 6 | 8 | 4 |
| labelme at 9c163b82 | 2 | 9 | 4 |
| imgviz, osam, attrs | 0 each | 1, 1, 2 | 2, 12, 2 |
| httpx | 43 | 0 | 1 |
| anyio | 25 | 3 | 78 |
| textual | 103 | 25 | 50 |
| trio | 126 | 40 | 61 |
| pytest | 184 | 57 | 264 |
| pydantic | 209 | 5 | 523 |

The repositories that already use underscored names inside private modules have few candidates left. labelme fixed its audit findings in wkentaro/labelme#2834. The public repositories follow ADR 0016's convention on purpose, so their findings are renames of working code, not defects. Hand-checked samples showed no finding that would block a review. The best case was a constant or helper whose plain name costs a search before it changes.

Many candidates are false positives or contested:

- Frameworks call some names by spelling. 179 of pytest's text-referenced candidates are pluggy hook implementations, and textual widget classes are CSS type selectors.
- Some public API is re-exported from another top-level package, such as `pytest` re-exporting `_pytest`. The re-export analysis in ADR 0016 only follows ancestor packages.
- TypeVars and type aliases account for 26 candidates in trio, 19 in pydantic, and 17 in textual. Whether they take an underscore is a style choice.
- Vendored code and test corpora, such as `gdown/_vendor` and pydantic's `tests/benchmarks/corpus` (116 candidates), are not the project's own naming.

The rule would also need a project-wide index of readers. Every rule today reads one file. With an index, `gruff check pkg/a.py` cannot see readers in files it was not given, so a partial run would flag names that other modules use.

A one-off audit script covers projects that want this convention. A deterministic rule does not need to.

Reconsider this only if ADR 0016 is revised to distinguish module privacy from package privacy, and a trial on repositories that follow that convention shows findings that would block a review.

## Prior requests

- [#109](https://github.com/wkentaro/gruff/issues/109): Flag plain names that only their own non-public module reads.
- [#91](https://github.com/wkentaro/gruff/issues/91): Proposed both directions; the import direction became GR014.
