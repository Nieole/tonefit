# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the actual label strings used in this repo's issue tracker.

| Label in mattpocock/skills | Label in our tracker | Meaning                                  |
| -------------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue  |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information |
| `ready-for-agent`          | `ready-for-agent`    | Fully specified, ready for an AFK agent  |
| `ready-for-human`          | `ready-for-human`    | Requires human implementation            |
| `wontfix`                  | `wontfix`            | Will not be actioned                     |

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding label string from this table.

Edit the right-hand column to match whatever vocabulary you actually use.

## How these appear in this repo

This repo's issue tracker is local markdown (`docs/agents/issue-tracker.md`), so there
are no tracker labels to apply. Write the label string on the `Status:` line near the
top of the issue file instead:

```markdown
# Screentone detection misfires on JPEG-transcoded sources

Status: needs-triage
```

One role per issue at a time — replace the value rather than accumulating them.

`resolved` is not one of the five roles: triage answers "should this be worked on", and the
five roles run out once the answer is yes. Delivered implementation tickets in this repo write
`Status: resolved` on that same line, alongside a `## 落地记录` section (see `issue-tracker.md`).
