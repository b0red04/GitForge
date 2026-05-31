# Runed Opportunity Audit — Reference

## Search strategy

Search the codebase systematically for these patterns.

### 1. Debouncing and throttling

**Search for:** `setTimeout`, `clearTimeout`, `setInterval`, `clearInterval`, related cleanup in `onDestroy` or effect cleanup.

**Runed utilities:** `Debounced`, `Throttled`, `useDebounce`, `useThrottle`, `useInterval`

### 2. DOM observers

**Search for:** `ResizeObserver`, `MutationObserver`, `IntersectionObserver`, manual observer cleanup.

**Runed utilities:** `useResizeObserver`, `useMutationObserver`, `useIntersectionObserver`, `ElementRect`, `ElementSize`

### 3. Event listeners

**Search for:** `addEventListener`, `removeEventListener`, manual teardown in `onDestroy` or effect cleanup.

**Runed utility:** `useEventListener`

### 4. Local storage

**Search for:** `localStorage.getItem`, `localStorage.setItem`, state persistence logic, cross-tab sync logic.

**Runed utility:** `PersistedState`

### 5. Click-outside detection

**Search for:** document click handlers, pointer or mousedown handlers, `element.contains(...)` containment checks.

**Runed utility:** `onClickOutside`

### 6. Scroll and resize handlers

**Search for:** window scroll listeners, window resize listeners, throttled or debounced viewport handlers.

**Runed utilities:** `ScrollState`, `useEventListener`, `Throttled`, `useThrottle`

### 7. Document visibility

**Search for:** `visibilitychange`, `document.hidden`, `document.visibilityState`.

**Runed utility:** `IsDocumentVisible`

### 8. Idle detection

**Search for:** timeout-based user activity tracking; mouse, keyboard, focus, or visibility listeners used to detect idle.

**Runed utility:** `IsIdle`

### 9. Animation frames

**Search for:** `requestAnimationFrame`, `cancelAnimationFrame`.

**Runed utility:** `AnimationFrames`

## Analysis process

For each pattern found:

1. Identify file path and relevant line numbers.
2. Inspect surrounding code to understand current behavior.
3. Check whether cleanup is manual, duplicated, fragile, or missing.
4. Assess priority using the guide in SKILL.md.
5. Estimate impact: lines, state variables, cleanup burden, risk reduction.
6. Skip cases already using Runed or not worth converting.

## Existing implementations to skip

Do not recommend converting these known Runed implementations:

- `src/routes/(app)/pay-trends/+page.svelte`
- `src/lib/components/charts/echarts-container.svelte`

Also avoid recommending opportunities already implemented with Runed elsewhere unless there is a clearly stronger replacement.

## Runed utility catalog

Use these when mapping opportunities.

### State management

`Debounced`, `Throttled`, `PersistedState`, `Previous`, `StateHistory`, `FiniteStateMachine`

### DOM observers

`useResizeObserver`, `useMutationObserver`, `useIntersectionObserver`, `ElementRect`, `ElementSize`, `IsFocusWithin`, `IsInViewport`

### Browser events

`useEventListener`, `onClickOutside`, `PressedKeys`, `ScrollState`

### Sensors

`IsDocumentVisible`, `IsIdle`, `useGeolocation`

### Utilities

`useDebounce`, `useThrottle`, `useInterval`, `AnimationFrames`
