# Gruff

Gruff is an opinionated, deterministic maintainability linter for Python. It complements Ruff with project policies that make agent-assisted code easier to understand and review; it does not infer who or what wrote the code.

## Installation

Requires Python 3.10 or later.

```bash
pip install gruff
```

Or with [uv](https://docs.astral.sh/uv/):

```bash
uv tool install gruff
```

Verify it works:

```bash
gruff --version
```

> [!TIP]
> To try the latest development version (the head of `main` on GitHub) before
> it is published:
>
> ```bash
> uv tool install git+https://github.com/wkentaro/gruff
> ```

## Quick start

Enable every Gruff rule in `pyproject.toml`:

```toml
[tool.gruff.lint]
select = ["GR"]
```

Then check the current directory:

```bash
gruff check .
```

All rules are opt-in. Use an exact code such as `GR001` to adopt rules individually; `GR` enables every Gruff rule. A check with no enabled rules succeeds but warns that it performed no policy analysis.

## Rules at a glance

The first release tests ten theses: inputs are easier to trace when definitions declare how callers pass them, non-public behavior is easier to review when callers supply every value, package initializer manifests are easier to review when every public import path defines `__all__`, constants are easier to review when uppercase names and `Final` annotations always appear together, non-public definitions are easier to understand when their names carry their purpose, comments are worth reading when they state more than the code beneath them, tests are worth running when an exception can still fail them, a body is easier to follow when its trailing condition inverts into a guard, a branch is easier to verify when its condition states the positive form, and a non-public module binding is easier to find when it lives in the one definition that reads it.

| Code | Rule | Policy |
| --- | --- | --- |
| GR001 | [`explicit-non-public-input-conventions`](https://wkentaro.github.io/gruff/rules/explicit-non-public-input-conventions/) | Every fixed input to a non-public callable has an explicit calling convention. |
| GR002 | [`required-non-public-inputs`](https://wkentaro.github.io/gruff/rules/required-non-public-inputs/) | Callers supply every fixed input to non-public callables. |
| GR003 | [`package-dunder-all`](https://wkentaro.github.io/gruff/rules/package-dunder-all/) | Every public package import path defines `__all__`. |
| GR004 | [`final-constants`](https://wkentaro.github.io/gruff/rules/final-constants/) | Uppercase names and `Final` annotations appear together. |
| GR005 | [`explicit-public-input-conventions`](https://wkentaro.github.io/gruff/rules/explicit-public-input-conventions/) | Every fixed input to a public callable has an explicit calling convention. |
| GR006 | [`no-non-public-docstrings`](https://wkentaro.github.io/gruff/rules/no-non-public-docstrings/) | Non-public definitions carry their purpose in their names instead of docstrings. |
| GR007 | [`no-subsumed-comments`](https://wkentaro.github.io/gruff/rules/no-subsumed-comments/) | One-line comments state something beyond the statements they annotate. |
| GR008 | [`no-exception-swallowing-tests`](https://wkentaro.github.io/gruff/rules/no-exception-swallowing-tests/) | Tests let exceptions propagate instead of swallowing them. |
| GR009 | [`no-guarded-tails`](https://wkentaro.github.io/gruff/rules/no-guarded-tails/) | Trailing conditions invert into guards instead of nesting the rest of the body. |
| GR010 | [`positive-branch-conditions`](https://wkentaro.github.io/gruff/rules/positive-branch-conditions/) | Branch conditions state the positive form instead of negating around an `else`. |
| GR011 | [`no-single-consumer-module-bindings`](https://wkentaro.github.io/gruff/rules/no-single-consumer-module-bindings/) | Non-public module bindings live in the one definition that reads them. |

Each rule links to its rule doc, which states what the rule flags, why, an example, and when to suppress. `gruff rule GR004` prints the same document in the terminal, and `gruff rule --all --output-format json` emits every rule for tooling.

## Documentation

The [documentation site](https://wkentaro.github.io/gruff/) carries the rule docs, and its [configuration page](https://wkentaro.github.io/gruff/configuration/) covers `pyproject.toml`, the command-line reference, suppressing findings, and the recommended Ruff pairing.

## Distribution

Gruff releases use PyPI wheels for Linux x86_64 and aarch64, macOS x86_64 and arm64, and Windows x86_64. Gruff is not published to crates.io.
