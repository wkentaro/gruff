# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

<!-- towncrier release notes start -->

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
