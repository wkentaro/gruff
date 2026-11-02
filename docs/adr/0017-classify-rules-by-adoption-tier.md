# Classify rules by adoption tier

Every rule carries one tier that states what adopting it costs. Baseline rules draw few objections, and fixing a finding changes code in one module only. Opinionated rules take a style position that some projects reject, and fixing a finding changes code inside the project but not how outside code calls it. API-shaping rules change public signatures, so fixing a finding can break callers outside the project. GR005 is the only API-shaping rule.

The quick start used to select `GR`, which put GR005 next to GR003. On osam, `GR` reported 278 findings: 68 were GR005, each a public signature change, and 9 were GR003, each a one-line `__all__`. A project that followed the quick start met its most expensive rules first. The quick start now selects the baseline rules, and the README and the site group the rule tables by tier.

The tier is rule metadata, so `gruff rule --output-format json` emits it and a test keeps each table row under its tier heading. Selection does not use tiers: a project still lists codes, and `GR` and `ALL` still enable every rule. A tier selector can follow if listing codes proves tedious. The tier assignments come from one repository trial and may move as trials accumulate.
