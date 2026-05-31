---
name: runed-opportunity-audit
description: Audits SvelteKit 2 + Svelte 5 codebases for manual browser/DOM/reactivity patterns replaceable by Runed utilities from runed.dev. Use when asked to audit for Runed opportunities, find manual debounce/observer/event-listener patterns, or refactor reactive boilerplate with Runed.
disable-model-invocation: true
---

# Runed Opportunity Audit

Find manual patterns in this codebase that [Runed](https://runed.dev) utilities can replace — reducing boilerplate, eliminating manual cleanup, and preventing memory leaks.

**Agent-driven only** — search with Grep and Read tools, inspect surrounding code, and reason about each hit. No bundled scripts.

## Quick start

1. Read [REFERENCE.md](REFERENCE.md) for search patterns and utility mapping.
2. Search the codebase for each pattern category using Grep (and Read for context).
3. Analyze hits; skip known Runed implementations (listed in REFERENCE.md).
4. Present findings using [OUTPUT-FORMAT.md](OUTPUT-FORMAT.md).
5. **Stop** — wait for the user to pick one opportunity before implementing.

## Operating mode

Two phases; **start in audit-only mode**:

| Phase            | Actions                                                                     |
| ---------------- | --------------------------------------------------------------------------- |
| **1. Audit**     | Search, analyze, report. Do not modify files or run repo-changing commands. |
| **2. Implement** | Only after the user explicitly selects an opportunity and asks to proceed.  |

## Audit workflow

- [ ] **Search** — use Grep for each pattern in [REFERENCE.md](REFERENCE.md): debounce/throttle, observers, listeners, storage, click-outside, scroll/resize, visibility, idle, animation frames. Read surrounding code before scoring.
- [ ] **Filter** — skip files already using Runed; skip known implementations in REFERENCE.md.
- [ ] **Analyze each hit** — record file:line, current behavior, cleanup burden, priority.
- [ ] **Estimate impact** — lines, state variables, manual cleanup, leak risk (before → after).
- [ ] **Report** — follow [OUTPUT-FORMAT.md](OUTPUT-FORMAT.md) with real code snippets from the codebase.
- [ ] **Recommend** — suggest one high-priority opportunity as the best first implementation.
- [ ] **Stop** — wait for the user to choose which opportunity to implement.

## Priority guide

| Priority  | Signals                                                                           |
| --------- | --------------------------------------------------------------------------------- |
| 🔴 High   | Multiple instances, manual cleanup, complex state, clear leak or maintenance risk |
| 🟡 Medium | Single instance with meaningful boilerplate or moderate cleanup benefit           |
| 🟢 Low    | Already reasonably abstracted, small code savings, limited practical benefit      |

Skip cases that already use a suitable Runed utility or are clearly not worth converting.

## Stop condition

After presenting the audit report, **stop and wait**. Do not implement unless the user explicitly chooses an opportunity.

## Advanced

- **Search patterns & Runed catalog**: [REFERENCE.md](REFERENCE.md)
- **Report template**: [OUTPUT-FORMAT.md](OUTPUT-FORMAT.md)
