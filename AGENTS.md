# Agent Behavior Guidelines

This file contains the mandatory rules for all autonomous agents operating in this repository. You must read and follow these instructions for all tasks, file changes, and git operations.

---

## Language and Writing Style

Use clear technical English for code comments, documentation, plans, logs, and commit descriptions. Follow these constraints:

*   **No Emojis**: Use alphanumeric text only in text generation, commits, and logs.
*   **Concise Sentences**: Prefer one concept per sentence. Keep sentences short.
*   **Active Voice**: Prefer `[Subject] + [Verb] + [Object]` structure. Prefer direct phrasing.
*   **Simple Tenses**: Prefer simple present, simple past, or imperative form.
*   **Precise Wording**: Use verbs for actions and nouns literally. Avoid vague modifiers.

---

## Required Workspace Directories & File Layout

You must only use the specified `.ai/` subdirectories to manage your workflow and state data. Below is the strict layout configuration you must follow and maintain:

```text
.ai/
├── handoff.md        # Real-time state transfer between agent iterations
├── plans/            # Active and historical execution strategies
│   ├── active.md     # Current multi-step plan for the assigned objective
│   └── history/      # Archived plans from completed milestones
├── artifacts/        # Final verified system components and code states
└── tmp/              # Scratchpads, raw engine outputs, and evaluation logs
```

### Directory Rules
*   `.ai/plans/`: Store task breakdowns, system designs, and execution steps here.
*   `.ai/artifacts/`: Store final generated assets, long-term outputs, and finished code modules here.
*   `.ai/tmp/`: Use this folder only for volatile data, temporary scratchpads, and execution logs.

> Note for agents: This doc covers boot state only. Create missing `.ai/` files and folders at startup. Do not expect them to exist in git.

---

## Repository Workflow and Branch Management

You must follow this versioning and branching process exactly. This workflow maintains a clean repository history.

### Version Number Format
The repository follows [Semantic Versioning](https://semver.org/) as `MAJOR.MINOR.PATCH`. This matches `Cargo.toml`.
*   Increment `PATCH` for fixes.
*   Increment `MINOR` for new features.
*   Increment `MAJOR` for breaking changes.

### Branch Creation Process
*   **Release Development Branch**: Create a `dev` branch off `main` for the next release.
*   **Feature Branches**: Branch feature tasks off `dev` using `feat/<name>` format.

### Integration and Merge Rules
*   **Feature to Dev Branch**: Feature branches can contain multiple intermediate commits. Use a squash merge into `dev`.
*   **Dev to Main Branch**: Use a squash merge to merge completed `dev` into `main`.

---

## Git Commit Standards

You must follow the Conventional Commits 1.0.0 specification for all commit messages. Do not write generic or unstructured commit summaries.

### Message Format Structure
```text
<type>[optional scope][!]: <subject>

[optional body]

[optional footer(s)]
```

### Allowed Message Types
*   `feat`: Adds a new feature, integration, or model capability.
*   `fix`: Patches a software bug, code syntax error, or prompt vulnerability.
*   `docs`: Updates documentation files only (such as markdown documentation).
*   `refactor`: Rewrites existing code without changing behavior or fixing bugs.
*   `test`: Adds, updates, or corrects verification tests and test suites.
*   `chore`: Updates build workflows, local configurations, or software package dependencies.

### Breaking Changes Rule
You must append a `!` directly after the type or scope if your change breaks backward compatibility or alters an existing API.
*   *Correct Example:* `feat(opencode)!: remove legacy execution environments`

---

## Changelog Standards

You must follow the Keep a Changelog 1.1.0 standard when you update the project `CHANGELOG.md` file. Do not copy raw git logs into the file.

### Change Categories
You must group your updates under these exact Markdown subheadings:
*   `Added` / `Changed` / `Deprecated` / `Removed` / `Fixed` / `Security`

### Formatting Rules
*   You must place the newest project version at the top of the changelog file. Use [Semantic Versioning](https://semver.org/) for the version header.
*   You must write release dates using the ISO 8601 standard format: `YYYY-MM-DD`.

---

## Agent Handoff Loop

You must record handoff data when you pass a task to another agent mesh. Write this data to the `.ai/handoff.md` file.

Use atomic writes for handoff updates. Avoid concurrent edits.

### Handoff Template
```markdown
# Handoff: [Task Name]
* **Current Status:** [1 sentence summarizing completed work]
* **Blocking Issues:** [None] OR [List active errors or missing contexts]
* **Next Actions:** [1-2 bullet points stating exactly what the next agent must do]
```

---

## Active Plan Template

When initializing a multi-step engineering task, populate tracking state inside `.ai/plans/active.md` using the exact layout below. Skip this for single-step tasks. Update checkboxes as milestones complete.

```markdown
# Active Execution Plan: [Feature/Task Name]

## Target Objective
[1-2 sentences in simple technical language describing the final desired system state.]

## Phase Breakdown

### Phase 1: Context & Pre-flight Verification
- [ ] Read and parse code modules related to the target scope.
- [ ] Run existing test suites to establish baseline pass rates.
- [ ] Verify execution boundaries within the isolated runtime container.

### Phase 2: Implementation & Logic Generation
- [ ] Generate modular code components inside `.ai/artifacts/`.
- [ ] Apply logic refactors to existing core repository modules.
- [ ] Write targeted validation unit tests for new behavior paths.

### Phase 3: Quality Assurance & Integration
- [ ] Execute runtime suite inside sandbox environment.
- [ ] Clean code files to remove trace telemetry or temporary logs.
- [ ] Format modifications to align with standard repository structure.

## Log of Deviations
* [None] - *If a system issue forces a detour from this plan, document the structural adjustment here immediately.*
```

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:46cd31e7 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   bd dolt push
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->

<!-- BEGIN BEADS CODEX SETUP: generated by bd setup codex -->
## Beads Issue Tracker

Use Beads (`bd`) for durable task tracking in repositories that include it. Use the `beads` skill at `.agents/skills/beads/SKILL.md` (project install) or `~/.agents/skills/beads/SKILL.md` (global install) for Beads workflow guidance, then use the `bd` CLI for issue operations.

### Quick Reference

```bash
bd ready                # Find available work
bd show <id>            # View issue details
bd update <id> --claim  # Claim work
bd close <id>           # Complete work
bd prime                # Refresh Beads context
```

### Rules

- Use `bd` for all task tracking; do not create markdown TODO lists.
- Run `bd prime` when Beads context is missing or stale. Codex 0.129.0+ can load Beads context automatically through native hooks; use `/hooks` to inspect or toggle them.
- Keep persistent project memory in Beads via `bd remember`; do not create ad hoc memory files.

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md for details and anti-patterns.
<!-- END BEADS CODEX SETUP -->
