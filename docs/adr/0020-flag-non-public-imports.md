# Flag non-public imports

GR014 flags a `from module import _name` statement when the module belongs to the same project. A leading underscore means a name is used only within its own module, and an import from another module is where that promise breaks. Ruff's PLC2701 skips imports within the same top-level package, and SLF001 covers only attribute access, so nothing caught `from pkg.a import _f` inside one project.

The policy has two halves. GR014 flags a non-public name used by another module, which is visible in the import statement. The other half flags a plain name in a private module that only its own module reads, which needs intent inference and exclusions. ADR 0005 makes that half an independently adoptable scope, so it gets its own code in #109 instead of an option on GR014.

GR014 decides by the imported name alone. Underscores in the module path do not count, and neither does a `_`-prefixed submodule imported from its package, because ADR 0016 makes a `_`-prefixed module private to its package, so a sibling importing it is correct use. A module is first-party when the import is relative or the absolute module resolves under the importing file's import root, the same root ADR 0016 computes. GR014 reuses the resolver that public re-export detection uses. An import that does not resolve on disk produces no finding, so the rule never guesses about third-party or standard-library modules. Test files, named as GR008 names them plus `conftest.py`, may import any name, and a test's import does not count as use outside the module.

On labelme before wkentaro/labelme#2834, GR014 reports 5 findings: `_round_bbox_to_int` imported by three `_automation` modules, and `_get_rotation_handles` and `_is_full_oriented_rectangle` imported by `_shape/_hit_testing.py`. All 5 are names that #2834 renamed by hand for this reason, so none is a false positive. A unit test that imports `_round_bbox_to_int` is exempt. On current labelme and imgviz, GR014 reports no findings.

GR014 is Baseline. A fix renames a name across modules, as a GR004 fix can, and the tier description says so.
