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
A development tool that extracts candidate review episodes from local Codex and Claude histories for agent and human evaluation of possible rules. Its output is temporary evidence, never a rule or finding.
_Avoid_: Linter, detector

**Candidate review episode**:
A non-initial direct-human turn between two agent code mutations in the same thread. It is input to semantic review, not yet a review correction.
_Avoid_: Correction, finding

**Review correction**:
An agent-produced code change made after explicit human feedback about earlier agent-produced code in the same review history.
_Avoid_: Feedback, edit
