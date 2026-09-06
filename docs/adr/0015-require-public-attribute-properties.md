# Require explicit public attribute properties

GR012 flags direct public instance-attribute stores in ordinary class methods,
including annotated, augmented, and non-`__init__` assignments. It accepts
underscore-prefixed storage, an explicit property getter, and a method
decorated as a property setter. The rule is opt-in and has no autofix because
renaming a field or adding a property can change an external contract.

The access half of the policy is delegated to Ruff `SLF001`
(`private-member-access`). Ruff already flags `logger._name` outside its
owning class and accepts `logger.name`; duplicating that finding in Gruff
would violate the repository's companion-linter boundary. GR012 therefore
owns storage declarations, while `SLF001` owns external private-member
access. Class-level declarations such as `ClassVar`, `__slots__`, and fields
declared for dataclass, attrs, or model frameworks are outside the direct
instance-store shape. An explicit `self.public = value` in a method is still
flagged when such a declaration exists, because a syntax-only rule cannot
prove that a framework or descriptor owns the assignment; the documented
`noqa` suppression is the deliberate escape for those contracts. Inherited
properties and runtime descriptors are not resolved.

The six reference repositories named by the repository's existing policy
trials were checked with the candidate rule on their default branches:

| Repository | GR012 findings |
| --- | ---: |
| `labelme` | 43 |
| `gdown` | 12 |
| `imgviz` | 0 |
| `osam` | 7 |
| `video-cli` | 0 |
| `imshow` | 11 |

The sweep was a syntax-only smoke test using `GR012` selection, followed by
manual review of each finding's surrounding class. Two unrelated parser
findings in `labelme` were excluded from that count. The review found
deliberate framework/model boundary writes, including dataclass field
normalization in `labelme`'s `Shape.__post_init__`, Qt widget state in
`labelme`, and plugin state in `imshow`; those remain explicit suppression
cases rather than guessed exceptions. Vendored and ordinary data-holder
classes in `gdown` and `osam` were retained as findings because their source
contains ordinary public instance stores and no syntax proves an external
framework owns the contract. No additional shape was promoted into the rule
because doing so would require import, inheritance, or runtime descriptor
inference. The key false-positive families covered by the regression fixture
are class constants, `ClassVar`, dataclass/slots declarations, property
setters, setter calls, static methods, and private backing storage.

Ruff `SLF001` is the established equivalent for the external access half;
Ruff has no rule that requires a public instance store to be paired with an
explicit property. The two rules therefore cover complementary halves of the
issue without overlapping diagnostics.
