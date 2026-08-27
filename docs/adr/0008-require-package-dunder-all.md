# Require package dunder all

GR003 flags a package initializer when a successfully completing path leaves public bindings without `__all__`. The bounded module-local analysis covers runtime and stub initializers, control flow, deletion, and normal Python binding forms without importing or executing the checked package. It emits only proven findings, leaves manifest contents to Ruff, and remains opt-in while repository trials establish that explicit package manifests reliably reduce review work.
