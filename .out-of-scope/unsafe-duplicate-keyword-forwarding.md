# Unsafe duplicate-keyword forwarding

A rule flagging potential duplicate-keyword failures in same-name positional-only forwarding is out of scope: a wrapper accepts a positional-only parameter and `**kwargs`, then passes an explicit keyword with that parameter's name alongside the forwarded mapping, such as `target(value=value, **kwargs)`.

## Why this is out of scope

A collision can intentionally reject a reserved key; runtime failure alone does not establish an accidental defect. The [evaluation in #81](https://github.com/wkentaro/gruff/issues/81#issuecomment-5563973445) covered 370 regular Python files at six pinned repository revisions and found no callables combining positional-only parameters with `**kwargs`, hence zero candidates. Precision is undefined (0/0). This provides insufficient evidence for a precise rule that flags only accidental failures.

Reconsideration requires recurring, independently verified accidental defects and a precise syntactic boundary evaluated against intentional rejection and unresolved cases. Renamed outgoing keywords and other value sources require separate evidence.

## Prior requests

- [#81](https://github.com/wkentaro/gruff/issues/81) — Evaluate unsafe duplicate-keyword forwarding.
