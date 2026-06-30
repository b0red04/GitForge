# ADR-0004: Operation Dispatch shell collapse

- **Status:** Accepted
- **Date:** 2026-06-30
- **Supersedes:** —
- **Relates to:** Architecture review Candidate 01 ("Collapse the 8-layer
  operation-dispatch shell stack"). Builds on the Operation Dispatch foundation
  described in `CONTEXT.md` → "Operation Dispatch".

## Context

`plan_dispatch` (the pure classifier in `dispatch.rs`) is the single decision
point for how an op's outcome reaches the user. CONTEXT.md framed the
surrounding GPUI-bound shells as a migration target: the legacy per-concern
seams (`run_git_op*`, `run_blocking_op_*`, `run_hosting_op`,
`dispatch_bg_result*`) were "being migrated onto" the classifier.

Surveying the migration's residue, **eight method shells** sat between the
~50 caller op-methods and `plan_dispatch`. Several were verbatim pass-throughs
— each doc comment literally read *"Thin adapter over …"*:

| Shell | Body | Callers |
|---|---|---|
| `run_op` | `run_op_full(…, None, None)` — defaults two args | **0 external** (both calls internal to the shell family) |
| `run_git_op_returning` | `run_git_blocking(…, OpEffects::QUIET, …)` — bakes in one constant | 8 |
| `run_git_op` | `run_git_blocking(…, OpEffects::GIT, …, \|_,_,_\|{})` — constant + empty closure | 26 |
| `run_git_op_with_status` | re-implements the handle guard, bypasses `run_git_blocking`, calls `run_op` | 5 |
| `run_hosting_op` | `run_op_full` + `anyhow → Remote` map | 7 |
| `run_blocking_op_returning` | `run_op_full` + `spawn_blocking` wrap, `OpEffects::QUIET` | 4 |
| `run_blocking_op_silent` | `run_op_full` + `spawn_blocking` wrap, `ErrorChannel::Silent` | 1 |
| `run_op_full` | **the real shell** — spawn + classifier routing | 5 bespoke staged ops + the adapters above |

Applying the deletion test:

- `run_op`, `run_git_op_returning`, `run_blocking_op_silent`: deleting moves
  complexity by zero characters (callers write a constant). **Pass-through.**
- The two `run_blocking_op_*` shells share a byte-identical `spawn_blocking`
  body; only the `OpEffects` constant differs. Earning their keep as a wrap,
  but not as two functions.
- `run_git_op_with_status`: **active divergence, not just shallowness.** It
  re-implements the handle guard from its sibling `run_git_blocking` but drops
  the `tab.loading` skip:

  ```rust
  // run_git_blocking (dispatch.rs) — the canonical guard
  let Some(handle) = self.repo_session.require_active_repo_handle() else { … };
  if self.repo_session.active_tab().is_some_and(|tab| tab.loading) {
      tracing::debug!("{label}: skipped, repo still loading");
      return;
  }
  // run_git_op_with_status (git_ops.rs) — copy-paste, missing the loading check
  let Some(handle) = self.repo_session.require_active_repo_handle() else { … };
  // … then calls run_op directly, bypassing run_git_blocking entirely
  ```

  So push/pull/fetch (its 5 callers) fire *during tab load* while
  stage/unstage correctly skip. A latent bug produced directly by the guard
  having no single home.

- `run_git_op` (26 callers): not a pure pass-through — it names the
  "fire-and-forget git op, no value consumed" category. Deletion would push an
  empty closure onto 26 call sites. Retained as one-line sugar.

The variance across the stack is three-axis:

- **(A) future construction** — sync git-op needing a handle (`with_repo_blocking`),
  sync repo-independent (`spawn_blocking`), async hosting (anyhow). Genuine
  variance; warrants a small fixed family.
- **(B) effects** — refresh flags, `remote_status`, error channel. **Already
  data** in `OpEffects` (with `QUIET` / `GIT` constants).
- **(C) callback shape** — `on_success`, optional `on_error` (detail-carrying or
  detail-dropping).

The shallow adapters existed mostly to bake a constant onto axis (B) — the
smell. Axis (A) is the legitimate reason to keep a small family.

## Decision

Collapse to **one shell (`run_op_full`) plus a small op-shape family (one
function per axis-A variant)**, with axes (B) and (C) carried as arguments.
Variance travels as data in `OpEffects`, not as a new shell.

### The surviving family

| Fn | Role | Signature shape |
|---|---|---|
| `run_op_full` | the shell — spawn + classifier routing + lifecycle | unchanged; bespoke staged ops (commit+push, 2× PR, 2× AI) call it directly |
| `run_git_blocking` | git op; **owns the single handle/loading guard**; takes `fx` | `on_success` only |
| `run_git_op` | fire-and-forget sugar | one-line delegator to `run_git_blocking(.., OpEffects::GIT, .., \|_,_,_\|{})` |
| `run_hosting_op` | async anyhow op | `on_error` is detail-carrying |
| `run_blocking` | the merged background helper | `fx` (carries Toast/Silent via `error_channel`); detail-carrying `on_error` |

### The guard gets a named, tested home

The handle/loading precondition is promoted from inline checks inside
`impl GitForgeApp` to a GPUI-free method on `RepoSession`:

```rust
enum GitOpReadiness {
    Ready(Arc<Mutex<Option<Repository>>>),
    NoRepo,
    Loading,
}
fn git_op_readiness(&self) -> GitOpReadiness;
```

`run_git_blocking` becomes one `match`:

```rust
match self.repo_session.git_op_readiness() {
    GitOpReadiness::Ready(handle) => self.run_op_full(
        label, cx, fx, move || with_repo_blocking(handle, op), on_success, None, None,
    ),
    GitOpReadiness::NoRepo => { tracing::warn!(..); self.push_toast(ToastKind::Warning, "No repository open", cx); }
    GitOpReadiness::Loading => { tracing::debug!(..); } // silent
}
```

Both pre-existing surfacing behaviours (Warning toast on no repo; silent on
loading) are preserved because the reason is discriminated. The decision and
the handle value fuse into one method.

`git_op_readiness` is unit-testable through the existing `RepoSession` test
fixture (`fake_tab(id, loading, has_state)`), matching the
`reselect_after_refresh` / `plan_dispatch` precedent of pure decisions living
in tested homes. It is distinct from `active_repo_ready` (which gates on
`repo_state.is_some()` for UI that reads the snapshot): the git-op path needs
the live repository handle, not the snapshot.

### Deletions (5)

`run_op` (0 external callers), `run_git_op_returning` (8 callers →
`run_git_blocking(.., OpEffects::QUIET, ..)`), `run_git_op_with_status`
(5 callers → `run_git_blocking(.., fx{remote_status}, ..)` — fixes the guard
divergence), `run_blocking_op_returning` and `run_blocking_op_silent` (folded
into `run_blocking`; `fx.error_channel` carries the Toast/Silent choice).

### `on_error` standardised on the detail-carrying shape

`run_hosting_op` and `run_blocking` take
`on_error: FnOnce(&mut Self, String, &mut Context<Self>)`, matching
`run_op_full`. Callers that don't need the detail write
`|this, _detail, cx| …`. One callback shape across the whole family; the
detail is always available when a caller later needs it.

## Scope boundary (explicit non-goals)

- **Bespoke staged ops still call `run_op_full` directly.** Commit+push, PR
  create/list/refresh, and AI generation need `on_error` / `finally` for
  multi-step orchestration and busy-flag lifecycle; they are the legitimate
  direct users of the full shell.
- **`run_git_blocking` stays `on_success`-only.** Its clientele is git porcelain
  that auto-toasts on error. A git op that needs a pre-toast `on_error` or a
  `finally` is, by definition, a staged op and routes through `run_op_full`.
- **The ~60 non-selection `self.repo_session.<panel>.<method>` reach-ins stay
  direct** (ADR-0003 scope). This ADR is about the dispatch shells, not panel
  façade behaviour.
- **Handle-less ops intentionally have no loading guard.** `run_blocking` and
  `run_hosting_op` do not check `tab.loading`: they don't touch the repo
  handle, and some (repo discovery/init) are the very ops that *set* loading.
- **`run_git_op` is retained as sugar.** Not the deepest possible outcome, but
  it names a high-traffic category (26 call sites) and its deletion would push
  an empty closure onto every one of them for no leverage gain.

## Behaviour changes

1. **push/pull/fetch now skip during tab load.** Folding
   `run_git_op_with_status` into `run_git_blocking` restores the `tab.loading`
   guard. `loading` is only true during initial tab open/discovery
   (`handle_open_repository`, `handle_new_tab`), so there is no "click Pull
   right after a refresh" regression — `refresh_repository` does not toggle
   loading, and the graph is not interactive while `loading` is true (no
   `repo_state`). This is the more defensive behaviour, matching the stance
   ADR-0003 took for selection.
2. **`run_hosting_op` / `run_blocking` callers' `on_error` gains a `String`
   detail parameter.** Mechanical: existing detail-dropping callers add
   `_detail`. The single `run_blocking_op_silent` caller already used the
   detail and is unchanged in behaviour.

## Consequences

### Positive

- **One guard, one spawn-blocking body.** The handle/loading precondition and
  the `spawn_blocking` + error-map wrap each live in exactly one place.
- **5 shallow adapters deleted.** 8 layers collapse to 3 (caller → op-shape
  helper → `run_op_full` → classifier).
- **`git_op_readiness` is named and tested.** The "Operation guard" invariant
  (previously enforced, divergently, at 2 untested sites) has a single home on
  `RepoSession`, unit-tested via `fake_tab`.
- **Variance as data.** Adding a new effect combination is a new `OpEffects`
  value at the call site, not a new shell. The path back to shell-proliferation
  is closed.
- **The live divergence is fixed.** push/pull/fetch no longer fire during tab
  load.

### Negative / deferred

- **`run_git_op` retained.** One shell survives for ergonomics over depth.
  Acceptable because it names a real category; revisitable if call-site style
  shifts.
- **`run_git_blocking` is `on_success`-only.** A git op needing `on_error` /
  `finally` must bypass to `run_op_full` directly. Deliberate (such an op is
  staged), but it means the op-shape family is not a complete substitute for
  the shell in all cases.
- **Two public readiness concepts on `RepoSession`** (`active_repo_ready` for
  snapshot UI, `git_op_readiness` for git ops). They agree on the `!loading`
  axis but differ on handle-vs-snapshot. Kept separate to avoid coupling the
  git-op path to snapshot state; documented here so a future merge is
  considered, not accidental.

## Verification

- `cargo build -p gitforge-app` — clean.
- `cargo clippy -p gitforge-app` — no warnings in the dispatch / ops modules.
- `cargo test --workspace` — green, including new `git_op_readiness` unit
  tests (`NoRepo` / `Loading` / `Ready` variants via `fake_tab`).
- Manual: open a repo tab and immediately trigger fetch — the op is skipped
  while loading; no Warning toast appears (silent `Loading` arm).
