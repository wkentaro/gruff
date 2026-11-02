# Classify rules by adoption tier

Every rule carries one tier that states what adopting it costs. Baseline rules draw few objections, and a fix usually stays inside one module. Opinionated rules take a style position that some projects reject, and a fix usually stays inside the project. API-shaping rules change public signatures, so every fix can break callers outside the project. GR005 is the only API-shaping rule.

A tier states the typical cost, not a guarantee. GR004 can rename a constant that other code imports, and GR012's getter-only property fix stops outside code from assigning the attribute. Both stay in their tiers because most of their findings do not touch an exported name.

The quick start used to select `GR`, which put GR005 next to GR003. On osam, `GR` reported 278 findings: 68 were GR005, each a public signature change, and 9 were GR003, each a one-line `__all__`. A project that followed the quick start met its most expensive rules first. The quick start now selects the baseline rules, and the README and the site group the rule tables by tier.

The tier is rule metadata, so `gruff rule --output-format json` emits it and a test keeps each table row under its tier heading. A `GR:<tier>` selector enables one tier, and the quick start selects `GR:baseline`. A tier is more specific than `GR` and less specific than any longer code prefix, so a project can drop a tier from `GR` or a code from a tier. Like `GR`, a tier selector picks up rules that join the tier in a later release, so upgrading gruff can add findings. The tier assignments come from one repository trial and may move as trials accumulate.
