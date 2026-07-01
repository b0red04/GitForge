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
`BusyFlag::should_clear_on_complete` holds), mirroring `remote_status`.

### Cleanup vs. result relevance

Two predicates on `BusyFlag` serve two distinct concerns:

- **`still_relevant(app)`** — result relevance. Callers gate **data writes** with
  this on a cloned `BusyFlag` so a stale response cannot clobber the active view
  (e.g. don't overwrite `hosting_repos` after the user switched providers).
- **`should_clear_on_complete(app)`** — lifecycle cleanup. `run_op_full` uses this
  to decide whether to clear the spinner flag when the request completes. For
  **shared-field** flags (e.g. `hosting_repos_loading`) this delegates to
  `still_relevant`, so a stale request cannot prematurely hide a newer request's
  loading state on the same field. For **per-owner** flags
  (`PullRequests { tab_id }`) this returns `true` unconditionally, because
  `set` targets the captured `tab_id` directly — the original tab's spinner must
  not get stuck just because the user navigated to a different tab mid-request.

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
| `PullRequests { tab_id, tab_path }` | `tab.pull_requests_loading` | active tab id + path (data write); cleanup always clears |

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
