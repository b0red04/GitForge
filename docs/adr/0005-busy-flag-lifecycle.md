# ADR-0005: Busy-flag lifecycle as data

- **Status:** Accepted
- **Date:** 2026-06-30
- **Supersedes:** —
- **Relates to:** Architecture review Candidate 03 ("Make the staged-op busy-flag
  lifecycle data"). Builds on ADR-0004 and `CONTEXT.md` → "Operation Dispatch".

## Context

ADR-0004 collapsed the operation-dispatch shell stack and fixed **guard
divergence** (`run_git_op_with_status` vs `run_git_blocking`). Bespoke staged
ops (hosting lists, PR flows, AI generation) still call `run_op_full` directly
and hand-write busy-flag lifecycle in 1–3 callbacks per op.

Surveying nine staged ops with UI spinners:

| Pattern | Ops | Risk |
|---|---|---|
| `finally` only | AI generate (commit message, branch name), PR AI title | Correct — one clear point |
| `on_success` + `on_error` both clear | hosting list/search, create-PR loads, refresh PRs, submit PR | Edit one arm → stuck spinner |

**15 mechanical `flag = false` closures** across those ops. The same divergence
class ADR-0004 flagged for git-op guards.

`OpEffects` already carries shell-owned lifecycle for `remote_status` (set
before spawn, clear in every arm). Busy flags were documented as "stay with the
shell" but were still scattered in caller closures.

## Decision

Carry busy-flag lifecycle as data in `OpEffects.busy: Option<BusyFlag>`.
`run_op_full` owns set-before-spawn and clear-on-every-outcome (when
`BusyFlag::still_relevant` holds), mirroring `remote_status`.

### `BusyFlag` enum

Lives in `crates/gitforge-app/src/views/ops/lifecycle.rs`. Each variant names
the spinner field and, when needed, a stale-response token captured at spawn:

| Variant | Field | Stale guard |
|---|---|---|
| `HostingRepos { expect_provider }` | `hosting_repos_loading` | `Some(p)` → active provider must match; `None` → unconditional |
| `AiGenerating` | `ai_generating` | none |
| `CommitPushGeneratingBranch` | `commit_push_generating_branch` | none |
| `CreatePrRepos(p)` | `create_pr.loading_repos` | `create_pr.provider == p` |
| `CreatePrBranches { provider, to_repo }` | `create_pr.loading_branches` | provider + `to_repo` |
| `CreatePrGeneratingAi` | `create_pr.generating_ai` | none |
| `CreatePrSubmitting` | `create_pr.submitting` | none |
| `PullRequests { tab_id, tab_path }` | `tab.pull_requests_loading` | active tab id + path |

`still_relevant` encodes the guard once. Success and error handlers clone the
same [`BusyFlag`] and gate data writes with `guard.still_relevant(app)` so
predicates cannot drift from the shell's flag clear.

### `run_hosting_op` takes `OpEffects` and optional `on_error`

Previously hardcoded `OpEffects::QUIET`. Callers without a busy flag pass
`OpEffects::QUIET` unchanged; callers without error-side cleanup pass
`on_error: None`.

## Scope boundary (explicit non-goals)

- **Cancel / validation paths** still clear flags manually (dialog cancel, early
  return before spawn when no account is configured).
- **`run_git_blocking`** remains busy-free; git porcelain uses `remote_status`
  where needed (e.g. commit+push).
- **Candidate 2** (AI/hosting generation skeleton) is orthogonal — shortens `op`
  bodies, not flag lifecycle.

## Consequences

### Positive

- **One home for busy lifecycle**, same precedent as `remote_status`.
- **15 mechanical closure clears deleted** across 9 call sites.
- **`BusyFlag::still_relevant` is unit-tested** via `TestAppContext` fixtures.
- **Divergence class closed** — no op can forget to clear on one outcome arm.

### Negative / deferred

- **`OpEffects` gains a GPUI-adjacent type** (`BusyFlag` references
  `GitForgeApp`). The classifier ignores `busy`; only the shell reads it.

## Verification

- `cargo build -p gitforge-app` — clean.
- `cargo test -p gitforge-app` — green, including new `lifecycle` tests.
- Manual: switch Add Repo account tab mid-fetch; change create-PR target repo
  mid branch-list; AI generate / submit PR spinner clears on success and error.
