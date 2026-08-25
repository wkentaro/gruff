# Ruffhouse

Ruffhouse provides deterministic Python policies that complement Ruff and reduce the review cost of agent-assisted code.

## Language

**Rule**:
A deterministic source-code policy identified by a stable `RH` code.
_Avoid_: Check, heuristic

**Finding**:
A source location where an enabled rule detects its prohibited code shape.
_Avoid_: Error, issue, violation

**Private call wrapper**:
A module-level private function with one direct caller, no other references, and one delegated call but no control flow. It may contain one call-free local binding that prepares the delegated call, but it does not establish a meaningful boundary.
_Avoid_: Thin function, forwarder

**Review miner**:
A development tool that extracts repeated corrections from existing agent review history to nominate possible rules. Its output is evidence for human evaluation, never a lint finding.
_Avoid_: Linter, detector
