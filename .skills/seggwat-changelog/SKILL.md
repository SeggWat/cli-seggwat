---
name: seggwat-changelog
description: Draft and publish SeggWat changelog entries from git history using the SeggWat MCP server. Trigger this skill whenever the user wants to write, draft, generate, or publish a changelog, release notes, or a "What's new" post from recent commits, merged PRs, or a release tag, keep the SeggWat changelog up to date, turn a release into customer-facing notes, close the loop on feedback that a release fixed, or wire changelog drafting into a release pipeline (GitHub Actions on tag push, headless `claude -p`). Also trigger on "what shipped since the last release", "write release notes for v1.4.0", or "update our public changelog". Covers picking the commit range, grouping Conventional Commits, writing customer-facing prose, creating draft entries, the explicit publish gate, duplicate checks, and resolving linked feedback.
---

# SeggWat Changelog

Turn a release (a tag or a commit range) into customer-facing entries on the project's public SeggWat changelog. Entries always land as **drafts**. They're published only after the user explicitly says so.

Changelog tools exist **only on the MCP server**. The `seggwat` CLI has no changelog commands. Git and `gh` supply the input.

## Connect

```bash
# OAuth: no token needed, run /mcp in Claude Code afterwards to sign in
claude mcp add --transport http seggwat https://seggwat.com/mcp

# Or a static Organization Access Token (headless, CI)
claude mcp add --transport http seggwat https://seggwat.com/mcp \
  --header "Authorization: Bearer $SEGGWAT_API_KEY"
```

## The entry model

| Field | Notes |
|---|---|
| `title` | Required. Short headline: "Export to Excel", not a commit subject |
| `body` | Markdown, rendered on the public board |
| `version` | Semver with a `v` prefix, taken from the release tag (`v1.4.0`). Pure version only, no date or codename |
| `category` | One of `added`, `improved`, `fixed`, `changed`, `removed`, `security` |
| `component` | Optional slug from the project's component registry (`cli`, `react-sdk`). An unknown slug errors and lists the valid ones |
| `status` | `draft` (default), `published`, or `hidden` |
| `source` | `manual` for entries you create. `github_release` and `completed_idea` drafts can appear on their own |

**One release = one entry per user-facing change, all with the same `version`** (and the same `component`, if the project uses components). The board renders them as one release block, with a category badge on each change. An entry holds only one category, so a single combined entry would lose the badges. If the user wants a single entry anyway, set its `category` to whichever kind of change dominates the release.

## Workflow

### 1. Resolve the project and check for existing entries

```
list_projects {}
list_changelog { "project_id": "<id>" }
list_changelog { "project_id": "<id>", "component": "cli" }   # monorepo with components
```

`list_changelog` returns every status, newest first. **If any entry already has this `version`** (and component), don't create new ones. Common case: the project syncs GitHub releases, so a `github_release` draft for the tag already exists. Offer to rewrite that entry with `update_changelog_entry` instead.

### 2. Pick the range

From a tag:

```bash
TAG=v1.4.0
PREV=$(git describe --tags --abbrev=0 "$TAG^")
git log --no-merges --format='%h %s%n%b' "$PREV..$TAG"
```

No tag yet: start from the newest **published** entry's `published_at` in `list_changelog`:

```bash
git log --no-merges --since="2026-09-12" --format='%h %s%n%b'
gh pr list --state merged --search "merged:>2026-09-12" --json number,title,body --limit 100
```

Then propose the next semver from that entry's `version` and the changes in range: a `!` or `BREAKING CHANGE:` bumps major, a `feat` bumps minor, only `fix`/`perf` bumps patch. Confirm the number with the user before using it as `version`.

### 3. Group by Conventional Commit type

| Commit | Category | Keep? |
|---|---|---|
| `feat` (new capability) | `added` | yes |
| `feat` (better existing capability), `perf` | `improved` | yes |
| `fix` | `fixed` | yes |
| `!` or a `BREAKING CHANGE:` footer | `changed`, or `removed` if something went away | always |
| security fix | `security` | always |
| `chore`, `ci`, `docs`, `test`, `build`, `refactor`, `style` | — | drop, unless users notice the change |

Merge commits that describe one user-visible change into a single entry. Drop anything internal-only.

### 4. Write for customers

- Say what changed **for the user**, and where to find it, in the present tense: "You can now export feedback as an Excel file from the Export menu."
- Don't include commit hashes, PR numbers, ticket ids, internal module names, or author names.
- Breaking changes say exactly what the user has to do.
- 1 to 3 sentences per entry. Bullets only when a change has several parts.

### 5. Create as drafts

Use one call per change. Never pass `publish: true`.

```
create_changelog_entry {
  "project_id": "<id>",
  "title": "Export to Excel",
  "body": "You can now export feedback as an `.xlsx` file from the **Export** menu, next to CSV.",
  "version": "v1.4.0",
  "category": "added"
}
```

Then show the user the full draft set as Markdown, grouped by category, with each entry's `id`.

### 6. Edit, then publish only on an explicit yes

Edits:

```
update_changelog_entry { "project_id": "<id>", "entry_id": "<eid>", "body": "<new markdown>" }
```

Only the fields you pass change. Pass `""` to clear `version`, `category`, or `component`.

Publish **only** after the user says to publish (for example "publish it" or "ship the v1.4.0 notes"). Approving the wording isn't approval to publish:

```
set_changelog_status { "project_id": "<id>", "entry_id": "<eid>", "status": "published" }
```

Call it once per entry. `draft` unpublishes, and `hidden` soft-hides without deleting.

### 7. Optional: close the loop on feedback

Find feedback the release addressed:

```bash
git log --format='%h %s%n%b' "$PREV..$TAG" | grep -iE 'feedback[: #]*[0-9a-f]{24}'
```

```
get_feedback  { "project_id": "<id>", "feedback_id": "<fid>" }               # confirm it exists and is still open
list_feedback { "project_id": "<id>", "search": "excel export", "status": "Active" }   # title-less match by keyword
```

Matching by keyword gives you **candidates** only. List them for the user, and don't act on them yourself.

Resolving **emails the submitter**, and the note goes into that email. So do it only after the entries are published and the user confirms the list:

```
update_feedback {
  "project_id": "<id>", "feedback_id": "<fid>",
  "status": "Resolved",
  "resolution_note": "Fixed in v1.4.0: large PNG uploads no longer crash. Thanks for reporting it!"
}
```

Write the note for the end user. Don't paste an internal id or a link you haven't verified. MCP returns no public changelog URL, so "see our changelog" is enough.

## Rules

- **Always draft first.** Never use `publish: true`, and never call `set_changelog_status` with `published` without an explicit instruction.
- **Check before creating.** Before writing anything, read `list_changelog`. An existing entry for the version means you update that entry instead of creating new ones.
- **One release block per version.** Every entry for a release shares `version` (and `component`).
- **Customer prose only.** Commit messages are input, not output.
- **Resolve feedback only after publishing, and only with confirmation.** It sends email.
- Never delete changelog entries or feedback. Use `hidden` for entries.

## Automate on release tags

This runs on every `v*` tag and leaves drafts for a human to publish. Commit this skill at `.claude/skills/seggwat-changelog/SKILL.md` so headless Claude Code finds it. Add the `ANTHROPIC_API_KEY` and `SEGGWAT_API_KEY` repository secrets.

```yaml
# .github/workflows/changelog-draft.yml
name: Changelog draft
on:
  push:
    tags: ['v*']

permissions:
  contents: read

jobs:
  draft:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0          # full history and tags for git describe/log

      - name: Install Claude Code
        run: |
          curl -fsSL https://claude.ai/install.sh | bash
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"

      - name: Draft SeggWat changelog
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
          SEGGWAT_API_KEY: ${{ secrets.SEGGWAT_API_KEY }}
          TAG: ${{ github.ref_name }}
        run: |
          cat > "$RUNNER_TEMP/mcp.json" <<EOF
          {"mcpServers":{"seggwat":{"type":"http","url":"https://seggwat.com/mcp",
            "headers":{"Authorization":"Bearer ${SEGGWAT_API_KEY}"}}}}
          EOF
          claude -p "Use the seggwat-changelog skill to draft changelog entries for tag $TAG in SeggWat project \"Acme Web\". Drafts only: do not publish and do not update feedback." \
            --mcp-config "$RUNNER_TEMP/mcp.json" \
            --allowedTools "Bash(git describe:*)" "Bash(git log:*)" "Bash(git tag:*)" \
              "mcp__seggwat__list_projects" "mcp__seggwat__list_changelog" "mcp__seggwat__create_changelog_entry"
```

`set_changelog_status` and `update_feedback` are left off the allow list, so CI can't publish or send email even if asked. A human reviews the drafts in the dashboard, or asks Claude to "publish the v1.4.0 notes".

## Worked example

**Input**: `git log --no-merges --format='%h %s' v1.3.2..v1.4.0`

```
a1b2c3d feat(export): add XLSX export
d4e5f6a fix(upload): handle PNGs over 5MB (closes feedback 66f1c0a2e4b0c9d1a2b3c4d5)
0a9b8c7 perf(dashboard): cache project stats
99aabbc feat(api)!: remove v0 endpoints
1122334 chore(deps): bump tokio to 1.47
5566778 ci: cache cargo registry
```

`list_changelog` shows no entry with `version: "v1.4.0"`. The `chore` and `ci` commits are dropped.

**Drafts created** (4 calls to `create_changelog_entry`, all `version: "v1.4.0"`):

```markdown
## v1.4.0 (draft)

**Added** · Export to Excel
You can now export feedback as an `.xlsx` file from the **Export** menu, next to CSV.

**Fixed** · Large image uploads
Uploading PNG screenshots larger than 5 MB no longer fails.

**Improved** · Faster dashboard
Project dashboards load noticeably faster, especially for projects with a lot of feedback.

**Removed** · Legacy v0 API
The v0 API endpoints are gone. If you still call `/api/v0/...`, switch to the
equivalent `/api/v1/...` endpoint. Request and response shapes are unchanged.
```

**Next, ask**: "4 drafts created for v1.4.0. Publish them? Commit `d4e5f6a` references feedback `66f1c0a2e4b0c9d1a2b3c4d5` (Active). After publishing, should I resolve it with the note 'Fixed in v1.4.0: large PNG uploads no longer fail'? That emails the submitter."

## Hit a problem with SeggWat itself?

```
report_problem { "category": "feature_gap", "title": "<one line>", "surface": "create_changelog_entry", "summary": "<what happened vs. expected>" }
```

`category` is one of `bug`, `docs_mismatch`, `friction`, `feature_gap`, `quality_degradation`, `other`. File one report per problem and keep going.
