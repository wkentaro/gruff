# Skip override methods

GR001, GR005, and GR013 skip a method decorated with `override`, from `typing` or `typing_extensions`. An override must accept the calls its base method accepts, so its convention belongs to the base class. Following a finding on the override alone breaks callers that hold the base type and makes type checkers report an incompatible override. On osam, 11 of the 69 GR013 findings were overrides of public `osam.types.Model` methods (`encode_image`, `default_prompt`, and `generate`) in private model modules.

The skip extends ADR 0018: a definition whose signature is declared elsewhere is outside the input convention policy. Gruff matches the decorator's spelling, as it does for framework decorators, so the check stays single-file. Detecting overrides without the decorator would mean resolving class hierarchies across modules, and `@override` already states the intent and lets type checkers verify it, so Gruff asks for the decorator instead.
