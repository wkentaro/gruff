# Triage labels

Labels record whose turn it is and what must happen next. A fresh issue has no triage label and is ready for an agent to route. Once triaged, each issue carries exactly one triage label and one type label.

## Issues

| Standard role | Tracker label | Meaning |
| --- | --- | --- |
| `needs-triage` | `needs-triage` | A maintainer must evaluate the issue |
| `needs-info` | `needs-info` | Waiting on the reporter for more information |
| `ready-for-agent` | `ready-for-agent` | Fully specified and ready for an AFK agent |
| `ready-for-human` | `ready-for-human` | Requires human implementation |
| `wontfix` | `wontfix` | Will not be actioned |

Use exactly one type label: `type: bug` for defects, `type: feature` for new capabilities or improvements, or `type: task` for maintenance, refactors, documentation, and other work.

## Pull requests

A draft PR is still being built. A non-draft PR without a verdict is ready for an agent to finalize. A PR waiting on an outside human uses `needs-info`.

Once finalized, a PR carries exactly one agent verdict:

| Verdict | Meaning |
| --- | --- |
| `recommend-merge` | The agent endorses the PR for maintainer review and merge |
| `recommend-close` | The agent actively recommends closing it because of a named technical, scope, abandonment, or supersession reason |
| `recommend-triage` | The code is sound, but a maintainer must make the product or scope decision |

`maintainer-approved` is a separate, human-only verdict: it records that a maintainer reviewed this head and approves merging after required checks pass. Apply it only at the maintainer's explicit direction. It may coexist with an agent verdict because the labels record decisions by different authorities.

Verdicts record decisions, not merge or close actions; agents do not merge or close PRs, and required checks remain authoritative. A new commit makes either authority's verdict stale; remove it and have that authority review the new diff before renewing it.
