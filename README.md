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

Enable the baseline rules in `pyproject.toml`:

```toml
[tool.gruff.lint]
select = ["GR002", "GR003", "GR004", "GR007", "GR008"]
```

Then check the current directory:

```bash
gruff check .
```

All rules are opt-in. Each rule belongs to a tier that states what adopting it costs. Start with the baseline rules, then add opinionated rules one code at a time. `GR` enables every Gruff rule, including the API-shaping tier. A check with no enabled rules succeeds but warns that it performed no policy analysis.

## Rules at a glance

The first release tests eleven theses: inputs are easier to trace when definitions declare how callers pass them, non-public behavior is easier to review when callers supply every value, package initializer manifests are easier to review when every public import path defines `__all__`, constants are easier to review when uppercase names and `Final` annotations always appear together, non-public definitions are easier to understand when their names carry their purpose, comments are worth reading when they state more than the code beneath them, tests are worth running when an exception can still fail them, a body is easier to follow when its trailing condition inverts into a guard, a branch is easier to verify when its condition states the positive form, a non-public module binding is easier to find when it lives in the one definition that reads it, and public instance data is easier to review when properties declare its interface.

### Baseline

Few projects disagree with these rules, and fixing a finding changes code in one module only.

| Code | Rule | Policy |
| --- | --- | --- |
| GR002 | [`no-uniform-non-public-defaults`](https://wkentaro.github.io/gruff/rules/no-uniform-non-public-defaults/) | Some calls in its module pass a module-private default and others omit it. |
| GR003 | [`package-dunder-all`](https://wkentaro.github.io/gruff/rules/package-dunder-all/) | Every public package import path defines `__all__`. |
| GR004 | [`final-constants`](https://wkentaro.github.io/gruff/rules/final-constants/) | Uppercase names and `Final` annotations appear together. |
| GR007 | [`no-subsumed-comments`](https://wkentaro.github.io/gruff/rules/no-subsumed-comments/) | One-line comments state something beyond the statements they annotate. |
| GR008 | [`no-exception-swallowing-tests`](https://wkentaro.github.io/gruff/rules/no-exception-swallowing-tests/) | Tests let exceptions propagate instead of swallowing them. |

### Opinionated

These rules take a style position that some projects reject. Fixing a finding changes code inside the project but not how outside code calls it.

| Code | Rule | Policy |
| --- | --- | --- |
| GR001 | [`explicit-non-public-input-conventions`](https://wkentaro.github.io/gruff/rules/explicit-non-public-input-conventions/) | Every fixed input to a non-public callable has an explicit calling convention. |
| GR006 | [`no-non-public-docstrings`](https://wkentaro.github.io/gruff/rules/no-non-public-docstrings/) | Non-public definitions carry their purpose in their names instead of docstrings. |
| GR009 | [`no-guarded-tails`](https://wkentaro.github.io/gruff/rules/no-guarded-tails/) | Trailing conditions invert into guards instead of nesting the rest of the body. |
| GR010 | [`positive-branch-conditions`](https://wkentaro.github.io/gruff/rules/positive-branch-conditions/) | Branch conditions state the positive form instead of negating around an `else`. |
| GR011 | [`no-single-consumer-module-bindings`](https://wkentaro.github.io/gruff/rules/no-single-consumer-module-bindings/) | Non-public module bindings live in the one definition that reads them. |
| GR012 | [`explicit-instance-data`](https://wkentaro.github.io/gruff/rules/explicit-instance-data/) | Public instance data is explicit through class-body annotations or properties. |

### API-shaping

Fixing a finding changes a public signature, which can break callers outside the project. On a published library, adopt these rules with a major release.

| Code | Rule | Policy |
| --- | --- | --- |
| GR005 | [`explicit-public-input-conventions`](https://wkentaro.github.io/gruff/rules/explicit-public-input-conventions/) | Every fixed input to a public callable has an explicit calling convention. |

Each rule links to its rule doc, which states what the rule flags, why, an example, and when to suppress. `gruff rule GR004` prints the same document in the terminal, and `gruff rule --all --output-format json` emits every rule and its tier for tooling.

## Documentation

The [documentation site](https://wkentaro.github.io/gruff/) carries the rule docs, and its [configuration page](https://wkentaro.github.io/gruff/configuration/) covers `pyproject.toml`, the command-line reference, suppressing findings, and the recommended Ruff pairing.

## Distribution

Gruff releases use PyPI wheels for Linux x86_64 and aarch64, macOS x86_64 and arm64, and Windows x86_64. Gruff is not published to crates.io.
