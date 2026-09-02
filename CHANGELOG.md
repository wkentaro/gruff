# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

<!-- towncrier release notes start -->

## 0.0.7 - 2026-09-02

### Fixed

- GR004 suppressions on multi-line assignments now belong beside the assigned name, matching the finding location and Ruff. ([#77](https://github.com/wkentaro/gruff/pull/77))


## 0.0.6 - 2026-09-02

### Added

- Added GR011 `no-single-consumer-module-bindings`, which flags a non-public, call-free module binding read by exactly one module-level function or method and asks for it to move into that definition. ([#75](https://github.com/wkentaro/gruff/pull/75))

### Fixed

- Aligned full-output caret lines for findings on rows 10+ and for source lines containing tabs or wide characters; tabs in the displayed source line now render as four spaces. ([#72](https://github.com/wkentaro/gruff/pull/72))
- Report rows and columns the way Ruff does: a lone carriage return now starts a new row — so `# noqa` scoping, the comment-subsumption window, and the guarded-tail line gate see each physical line on such files — and a byte-order mark counts toward neither the first row's columns nor its printed source line. ([#73](https://github.com/wkentaro/gruff/pull/73))


## 0.0.5 - 2026-09-01

### Added

- `gruff rule` prints the rule doc for a rule code or name, with `--all` and `--output-format text|json`, and the same rule docs ship as a documentation site at https://wkentaro.github.io/gruff/. ([#56](https://github.com/wkentaro/gruff/pull/56))
- Added opt-in GR007 to flag one-line comments subsumed by the statements they annotate. ([#57](https://github.com/wkentaro/gruff/pull/57))
- Added opt-in GR008 to flag exception handlers in tests that only pass, return, or skip, leaving the test unable to fail. ([#60](https://github.com/wkentaro/gruff/pull/60))
- Added opt-in GR009 to flag a trailing else-less `if` that nests the rest of a function or loop body instead of inverting into a `return` or `continue` guard. ([#64](https://github.com/wkentaro/gruff/pull/64))
- Added opt-in GR010 to flag an `if` statement with a plain `else` whose condition is negated, where swapping the branches states the condition positively. ([#69](https://github.com/wkentaro/gruff/pull/69))

### Changed

- Aligned `# noqa` lexing with Ruff on the common forms: directives may follow other comment text or a doubled hash, code lists end at the first non-code token, and rule codes now match case-sensitively. ([#57](https://github.com/wkentaro/gruff/pull/57))

### Fixed

- Fixed full output rendering a single mispointed caret for a finding whose range spans lines; the underline now extends to the end of the start line. ([#66](https://github.com/wkentaro/gruff/pull/66))
- GR001, GR002, GR005, and GR006 no longer flag methods of a class defined inside a function; like nested functions, those methods sit outside the definition concept. ([#67](https://github.com/wkentaro/gruff/pull/67))
- Fixed the superquadratic `# noqa` lookup that made comment-dense files take tens of seconds to check; suppression and offset lookups now resolve through a shared per-file line index. ([#71](https://github.com/wkentaro/gruff/pull/71))


## 0.0.4 - 2026-08-29

### Added

- Added opt-in GR006 to prohibit docstrings on non-public definitions. ([#51](https://github.com/wkentaro/gruff/pull/51))

### Changed

- Split explicit input conventions into independently selectable non-public GR001 and public GR005 rules, and aligned GR002 with the non-public definition boundary. ([#42](https://github.com/wkentaro/gruff/pull/42))


## 0.0.3 - 2026-08-28

### Changed

- Changed GR001 to cover every module-level function or method and renamed it to explicit-input-conventions. ([#38](https://github.com/wkentaro/gruff/pull/38))

### Fixed

- Fixed GR001's rule name and diagnostic to describe explicit private input calling conventions. ([#37](https://github.com/wkentaro/gruff/pull/37))


## 0.0.2 - 2026-08-28

### Changed

- Changed GR001 to accept fixed positional-only private inputs as an explicit calling convention. ([#32](https://github.com/wkentaro/gruff/pull/32))


## 0.0.1 - 2026-08-27

### Added

- Added a Ruff-compatible command-line interface with deterministic configuration, diagnostics, and exit statuses.
- Added the opt-in GR001 keyword-only private inputs, GR002 required private inputs, GR003 package dunder all, and GR004 final constants rules.
- Added Python 3.10+ wheels for Linux x86_64 and aarch64, macOS x86_64 and arm64, and Windows x86_64.
