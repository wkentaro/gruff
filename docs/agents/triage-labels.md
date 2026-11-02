# Triage labels

Labels record whose turn it is and what must happen next. This file maps each `/triage` role to the label string an agent applies.

| Role | Issue label | PR label |
| --- | --- | --- |
| `bug` | `type:bug` | none; the conventional-commit title carries the type |
| `enhancement` | `type:feature`, or `type:task` for maintenance, refactors, docs, and other work | none; the conventional-commit title carries the type |
| `needs-triage` | `needs-triage` | `recommend-triage` |
| `needs-info` | `needs-info` | `recommend-revise` |
| `ready-for-agent` | `ready-for-agent` | no verdict label on a non-draft PR |
| `ready-for-human` | `ready-for-human` | `recommend-merge` |
| `wontfix` | `wontfix` | `recommend-close` |

## Issues

An unlabeled issue is untriaged. `needs-triage` means a maintainer is evaluating it and has not routed it yet. A triaged issue carries exactly one `type:` label and one triage state.

## Pull requests

The draft flag means the PR is still being built; no label says that. A non-draft PR with no verdict is the agent's to finalize.

The agent sets at most one verdict per head:

| Verdict | Meaning |
| --- | --- |
| `recommend-merge` | The agent endorses the PR for maintainer review and merge |
| `recommend-close` | The agent recommends closing it for a named technical, scope, abandonment, or supersession reason |
| `recommend-triage` | The code is sound, but a maintainer must make the product or scope decision |
| `recommend-revise` | The review found defects or open questions, and the author must revise and push |

`recommend-revise` hands the PR back to its author. The other three hand it to the maintainer.

`maintainer-approved` is a separate, human-only verdict. It records that a maintainer reviewed this head and approves merging after required checks pass. Apply it only at the maintainer's explicit direction. It may coexist with a `recommend-*` label because the two labels record decisions by different authorities.

Verdicts are recommendations. Agents never merge or close PRs, and required checks stay authoritative. Never rename `recommend-merge` to `ready-to-merge`, because merge bots watch that string. A new push makes any verdict stale, and the authority that set it clears it and reviews the new diff before renewing it.
