# Gruff

Gruff provides deterministic Python policies that complement Ruff and reduce the review cost of agent-assisted code.

## Language

**Rule**:
A deterministic source-code policy identified by a stable `GR` code.
_Avoid_: Check, heuristic

**Finding**:
A source location where an enabled rule detects its prohibited code shape.
_Avoid_: Error, issue, violation

**Non-public definition**:
A module-level function or method whose name starts with an underscore and does not end with an underscore. This includes `_name` and `__name` definitions; double-leading names are name-mangled in class scope. Nested functions, methods of a class defined inside a function, ordinary names, trailing-underscore names, sunder protocol names, and dunder methods are outside this term.
_Avoid_: Private definition, internal callable, underscore function

**Public definition**:
Gruff's syntactic complement of a non-public definition. It includes ordinary names, trailing-underscore names, framework or protocol sunder names, and system-defined dunder methods; it does not assert that an interface is documented or exported.
_Avoid_: Exported definition, API definition

**Explicit input calling conventions**:
A policy requiring every non-receiver, non-variadic input to a module-level function or method to be positional-only or keyword-only, so its calling convention is locally declared and runtime-enforced.
_Avoid_: Kwargs-only definitions, named inputs

**Required non-public inputs**:
A policy requiring every non-receiver, non-variadic input to a non-public definition to have no default, so callers supply each value.
_Avoid_: Required private inputs, no non-public defaults, explicit non-public inputs

**Package dunder all**:
A policy requiring every successfully completing package-initializer path with a binding whose name does not start with an underscore to finish with `__all__` bound.
_Avoid_: Explicit exports, package exports

**Final constants**:
A policy requiring every simple-name constant binding to pair an uppercase name with a `Final` annotation in every lexical scope.
_Avoid_: Uppercase variables, final variables

**Comment subsumption**:
A lexical policy identifying a one-line own-line comment whose content words are all present in the window it annotates, the next line carrying code, with comments counting as blank, plus the three physical lines after it, after fixed stopword and synonym handling.
_Avoid_: Obvious comment, redundant comment, narrative-comment heuristic

**Exception swallowing test**:
A policy identifying an `except` clause inside a test definition whose body statements are each only a `pass`, an `...`, a bare `return`, or a skip call, after an optional docstring, so no exception it catches can fail the test; an `else` clause on the `try` exempts such a handler only when it makes no skip call. A test definition is a function or method whose name starts with `test` in a file named `test_*.py` or `*_test.py` and not lexically nested inside another function's body.
_Avoid_: Empty except, silent failure, missing-assertion heuristic

**Guarded tail**:
A policy identifying an `if` statement with no `elif` or `else` that is the last statement directly in a function body or loop body and whose suite spans at least ten physical lines or lexically contains an `if` statement, so the rest of the body nests inside the condition instead of a `return` or `continue` guard.
_Avoid_: Trailing conditional, guard-clause heuristic, early-return rule

**Positive branch conditions**:
A policy identifying an `if` statement carrying a plain `else` and no `elif` whose test's outermost operation is a `not`, or a single-comparator `is not`, `!=`, or `not in` comparison, so the branches swap to state the condition positively.
_Avoid_: Negated condition check, inverted if, condition polarity heuristic

**Single-consumer binding**:
A non-public module binding — a direct child of the module body assigning a plain single-underscore-prefixed, non-trailing-underscore name to a value that is neither a call expression nor an empty list or dict display — that is bound nowhere else in the module, is read at least once, and whose every read sits inside the body of exactly one consumer, a `def` that is a direct child of the module body or of a module-level class body, with reads from nested functions, lambdas, and comprehensions counting as the consumer's. A read in a module-level statement, a class body, or a consumer's decorators, defaults, or annotations, a subscript or attribute store or delete whose chain leads back to the name, a name listed in `__all__`, or a module that reads `globals`, `vars`, `eval`, or `exec` by name keeps the binding at module scope.
_Avoid_: Hoisted constant, module-level constant used once, private constant scoping

**Rule doc**:
The canonical document for one rule, with four fixed sections: what it does, why, an example, and when to suppress. Every surface that explains a rule presents this document unchanged. The `explanation` key of `gruff rule --output-format json` is the one licensed exception; it follows Ruff's JSON shape but carries the whole rule doc, title included.
_Avoid_: Rule page, rule explanation

**Review miner**:
A development tool that extracts candidate review episodes from local Codex and Claude histories for agent and human evaluation of possible rules. Its output is temporary evidence, never a rule or finding.
_Avoid_: Linter, detector

**Candidate review episode**:
A non-initial direct-human turn between two agent code mutations in the same thread. It is input to semantic review, not yet a review correction.
_Avoid_: Correction, finding

**Review correction**:
An agent-produced code change made after explicit human feedback about earlier agent-produced code in the same review history.
_Avoid_: Feedback, edit
