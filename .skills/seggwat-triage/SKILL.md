---
name: seggwat-triage
description: Triage new SeggWat feedback with the SeggWat MCP server or the `seggwat` CLI. Trigger this skill whenever the user wants to triage, classify, sort, clean up, review, or work through new or incoming feedback, fix wrongly typed feedback (bugs filed as "Other"), find or merge duplicate feedback, close spam, run a feedback review or inbox-zero pass, or set up a recurring or scheduled triage loop (cron, CI, `/loop`, `/schedule`). Also trigger on "what came in since yesterday", "go through the feedback inbox", or "triage SeggWat". Covers resolving the project, paging through New feedback, retyping, duplicate detection, status changes with notes, idempotent re-runs, and a Markdown triage summary.
---

# SeggWat Triage

A repeatable loop that works through a project's **New** feedback: fix the type, catch duplicates and spam, move each item to the right status, and report what still needs a human.

Works over the SeggWat MCP server (preferred inside an agent) or the `seggwat` CLI (cron and CI). Both hit the same API.

## Connect

### MCP (Claude Code)

```bash
# OAuth: no token needed, run /mcp in Claude Code afterwards to sign in
claude mcp add --transport http seggwat https://seggwat.com/mcp

# Or a static Organization Access Token (headless, CI)
claude mcp add --transport http seggwat https://seggwat.com/mcp \
  --header "Authorization: Bearer $SEGGWAT_API_KEY"
```

`X-API-Key: oat_...` works as the header too. Tokens are created under Settings > API Tokens and need admin or owner access.

### CLI

```bash
cargo install seggwat-cli
export SEGGWAT_API_KEY=oat_<your-token>
```

## What the API gives you

Fields on each feedback item: `id`, `project_id`, `message`, `status`, `type`, `source`, `created_at`, `path`, `version`, `submitted_by`, `resolution_note`, `archived`. There is no title, so judge on `message`.

| Status | Use in triage |
|---|---|
| `New` | Untriaged. Also where items stay when you're unsure |
| `Active` | Actionable, accepted for work |
| `Hold` | Blocked or needs more info from the submitter |
| `Closed` | Spam, noise, or duplicate (with a note) |
| `Resolved` | Already fixed. **Emails the submitter** with the note, so triage never sets it |

Types: `Bug`, `Feature`, `Praise`, `Question`, `Improvement`, `Other`.

Limits to design around:
- **Don't use `Assigned`.** The server no longer has it. Over MCP it's silently dropped from `update_feedback`, and as a `list_feedback` filter the request fails.
- There are no tags, priority, or assignee fields over MCP or the CLI, so put priority in the summary, not on the item.
- There is no date filter. Lists come back newest first, so to get "since X", stop paging once `created_at` is older than X.
- `update_feedback` silently ignores a misspelled type or status. Check the returned item.

## Workflow

### 1. Resolve the project

```
list_projects {}
```

Match by name. If more than one project matches, ask the user which one. Don't guess.

### 2. Read everything first

Get all New items, page by page, until `pagination.page == pagination.total_pages`:

```
list_feedback { "project_id": "<id>", "status": "New", "limit": 100, "page": 1 }
```

Status and type filter values are case-sensitive: use `New`, not `new`.

Then load the open items you'll compare against for duplicates. Open means `Active` and `Hold`, since the status filter takes only one value:

```
list_feedback { "project_id": "<id>", "status": "Active", "limit": 100 }
list_feedback { "project_id": "<id>", "status": "Hold", "limit": 100 }
```

Finish all reads before your first write.

### 3. Decide per item

For each New item, decide in this order:

1. **Spam or empty** (gibberish, ads, a test like "asdf"): move to `Closed` with `resolution_note: "Closed in triage: spam"`.
2. **Duplicate**: it describes the same problem or request as an open item, or as an older New item in this batch. The original is the oldest one. Move the newer item to `Closed` with `resolution_note: "Duplicate of <original-id>"`. If you can't name the original's id, it's not a duplicate.
3. **Retype**: if `type` is `Other` or plainly wrong, set the right one. Examples: "crashes when I upload" is a `Bug`, not a `Feature`; "please add dark mode" is a `Feature`; "how do I export?" is a `Question`.
4. **Status**:
   - Clear and actionable: `Active`.
   - Actionable but missing what you need (no steps to reproduce, unclear page): `Hold`.
   - `Praise` with nothing to act on: `Closed` with note `"Closed in triage: praise, no action needed"`.
   - You aren't confident: **change nothing** and leave it in `New`. List it under "Needs a human".

### 4. Write

Send one call per changed item, with only the fields that change:

```
update_feedback { "project_id": "<id>", "feedback_id": "<fid>", "type": "Bug", "status": "Active" }
update_feedback { "project_id": "<id>", "feedback_id": "<fid>", "status": "Closed", "resolution_note": "Duplicate of 66f1c0a2e4b0c9d1a2b3c4d5" }
```

If the item already has the target type and status, skip the call.

### 5. Summarize

Print a short Markdown summary (template in the worked example below). Don't post it anywhere unless the user asks.

## Rules

- **Never delete.** Don't call `delete_feedback` or `seggwat feedback delete` during triage, not even for spam. Close it instead.
- **Never set `Resolved`.** It emails the submitter. Resolving belongs to whoever shipped the fix.
- **No duplicate without an id.** The note must contain the original's id.
- **Unsure means untouched.** Leave the item in `New` and list it in the summary.
- **Read everything, then write.** Classification can change once you've seen the whole batch, especially for duplicates.
- **Idempotent.** Only `New` items are processed, and you skip any write that wouldn't change anything. A second run right after the first makes zero writes and lists the same "Needs a human" items.
- Don't edit `message`. It's the submitter's words.

## CLI equivalents

Use these for cron and CI. CLI values are lowercase. Add `--json` for scripting.

```bash
seggwat project list --json | jq -r '.projects[] | "\(.id)\t\(.name)"'

seggwat feedback list "$PROJECT_ID" --status new --limit 100 --page 1 --json \
  | jq '{page: .pagination.page, total_pages: .pagination.total_pages, items: [.feedback[] | {id, type, created_at, message}]}'

seggwat feedback list "$PROJECT_ID" --status active --limit 100 --json
seggwat feedback list "$PROJECT_ID" --status hold --limit 100 --json

seggwat feedback update "$PROJECT_ID" "$FID" --type bug --status active
seggwat feedback update "$PROJECT_ID" "$FID" --status closed --resolution-note "Duplicate of $ORIGINAL_ID"
```

The CLI still offers `--status assigned`, but the server rejects it, so don't use it.

If `seggwat feedback list` fails with a parse error, the project has feedback from a source your CLI build doesn't recognize (for example App Store, survey, or agent reports). Use the MCP path for that project.

## Run it on a schedule

**In a Claude Code session**, re-run every few hours while the session is open:

```
/loop 4h Use the seggwat-triage skill on project "Acme Web"
```

**As a cloud routine**, so it runs without your laptop:

```
/schedule every weekday at 8am: use the seggwat-triage skill on project "Acme Web" and post the summary
```

**As cron or CI**, headless with the CLI. Allow only the commands triage needs:

```bash
export SEGGWAT_API_KEY=oat_<your-token>
claude -p 'Use the seggwat-triage skill on SeggWat project "Acme Web" via the seggwat CLI. Print the triage summary.' \
  --allowedTools "Bash(seggwat project list:*)" "Bash(seggwat feedback list:*)" \
                 "Bash(seggwat feedback get:*)" "Bash(seggwat feedback update:*)" "Bash(jq:*)"
```

Leaving `feedback delete` off the allow list enforces the never-delete rule.

## Worked example

**Input**: `list_feedback { status: "New" }` returns 6 items. Open items include `66f1a0...01` (Active, Bug): "CSV export times out for large projects".

| id | type | message |
|---|---|---|
| `...a1` | Other | "Export to CSV just spins forever, 3k rows" |
| `...a2` | Feature | "App crashes when I upload a PNG over 5MB" |
| `...a3` | Other | "BUY CHEAP FOLLOWERS http://..." |
| `...a4` | Praise | "Love the new dashboard!!" |
| `...a5` | Question | "Can I invite teammates on the Starter plan?" |
| `...a6` | Other | "the thing on the page is weird" |

**Actions**:

```
update_feedback { feedback_id: "...a1", type: "Bug", status: "Closed", resolution_note: "Duplicate of 66f1a0...01" }
update_feedback { feedback_id: "...a2", type: "Bug", status: "Active" }
update_feedback { feedback_id: "...a3", status: "Closed", resolution_note: "Closed in triage: spam" }
update_feedback { feedback_id: "...a4", status: "Closed", resolution_note: "Closed in triage: praise, no action needed" }
update_feedback { feedback_id: "...a5", status: "Active" }
# ...a6: no call, too vague to classify
```

**Summary**:

```markdown
## Triage: Acme Web (2026-10-09)

Processed 6 New items: 5 updated, 1 left for a human.

| Type | Count |
|---|---|
| Bug | 2 (1 retyped from Feature, 1 from Other) |
| Question | 1 |
| Praise | 1 |
| Other | 2 |

**Moved to Active (2)**
- `...a2` Bug: crash on PNG upload over 5MB. Likely high priority
- `...a5` Question: teammate invites on Starter

**Closed (3)**
- `...a1` Duplicate of `66f1a0...01` (CSV export timeout)
- `...a3` Spam
- `...a4` Praise, no action

**Needs a human (1)**
- `...a6` "the thing on the page is weird". No path or details, left in New
```

## Hit a problem with SeggWat itself?

If a tool errors unexpectedly or behaves differently from what's described here, file it once and keep going:

```
report_problem { "category": "bug", "title": "list_feedback rejects status filter", "surface": "list_feedback", "summary": "<what happened vs. expected>" }
```

`category` is one of `bug`, `docs_mismatch`, `friction`, `feature_gap`, `quality_degradation`, `other`. The report goes to the SeggWat team, not to your project.
