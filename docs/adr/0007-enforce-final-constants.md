# Enforce final constants

GR004 flags simple-name assignments whose uppercase spelling and `Final` annotation do not appear together. The pairing applies in module, class, and function scopes, including nested control flow, while enum members, type aliases, and non-simple assignment forms are excluded. The 2026-08-27 specification sweep found 119 uppercase bindings without `Final` and 6 `Final` bindings without uppercase names across the six reference repositories. The rule uses syntax only, without import-alias resolution or dataflow, and remains opt-in while repository trials establish that one constant spelling reliably reduces review work.

A class-body `Final` annotation with no value, such as `_device: Final[str | None]`, is exempt. It declares an instance attribute that a method assigns, the form GR015 asks for when a method stores a `Final` field, so requiring an uppercase name would make the two rules contradict each other.
