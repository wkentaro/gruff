# Decide non-public definitions by scope

GR001, GR002, GR005, and GR006 decide whether a definition is non-public from its enclosing scopes as well as its own name. An ordinary name is non-public when an enclosing class, its module, or a package between the module and its import root starts with an underscore and does not end with one. A leading underscore then means one thing at every level: private to the scope that encloses it. A project marks privacy once, where it starts, instead of repeating it on every name inside a private module.

The name-only boundary let a definition leave the non-public rules by dropping its underscore, and a project that marks its modules private has no reason to keep one on names those modules share. On labelme, where every module is `_`-prefixed, deciding by scope adds 61 findings: 49 for GR002, 9 for GR006, and 3 for GR001.

This widens GR002 and GR006, which ADR 0005 would give new codes. It does not here: a second code per rule would encode two meanings of non-public, so every rule that reads the boundary would need a twin and every user would choose between them. The boundary is one term in `CONTEXT.md`, so it changes once, as a breaking change, while these rules stay opt-in. ADR 0005 still governs scope expansions that do not redefine a shared term.

Dunder, sunder, and trailing-underscore names keep the public classification inside a private scope, because their spelling is a protocol or keyword contract rather than a visibility choice. The import root comes from `__init__.py` and `__init__.pyi` files on disk, so a checkout under a `_`-prefixed directory, or a namespace package without an initializer, adds no privacy. GR011 keeps the name-only boundary: it reads one module at a time and cannot see a sibling module import a plain name, which is exactly what a plain name in a private module is for.

## Direct package re-exports

Imgviz implements its documented public API in `_`-prefixed modules and re-exports it from public package initializers. The scope boundary alone classified 53 API docstrings as non-public and assigned 335 public parameters to GR001. Deleting those docstrings or treating their signatures as freely changeable would lose the library's documentation or break callers.

An unconditional direct re-export into a public ancestor package now overrides the defining scope for that definition and the ordinary methods of a re-exported class. Static `__all__` manifests constrain the names; without one, imported names without a leading underscore are public. GR005 owns their input conventions and GR006 leaves their docstrings alone. The shared definition changes once, as in the original scope decision, rather than introducing a second set of rule codes.

The analysis stays bounded to direct ancestor-package imports and literal manifests. It does not execute Python or follow forwarding modules. Unresolved exports keep the name-and-scope classification, with targeted suppressions available for dynamic APIs.
