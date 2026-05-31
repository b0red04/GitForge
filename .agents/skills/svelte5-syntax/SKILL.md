---
name: svelte5-syntax
description: Svelte 5 syntax reference enforcing runes ($state, $derived, $effect, $props) and modern patterns. Use when writing ANY .svelte component to avoid deprecated Svelte 4 patterns.
license: MIT
compatibility: opencode
metadata:
  audience: frontend-devs
  enforce: style
---

# Svelte 5 Syntax & Runes

Enforce the new Svelte 5 syntax (runes) in every component. This skill helps agents write modern, correct Svelte code.

## Core Runes

### `$state` - Reactive State

```svelte
<script lang="ts">
	let count = $state(0);
	let user = $state<User | null>(null);

	// For objects/arrays, the proxy tracks nested changes
	let items = $state<string[]>([]);
</script>
```

### `$derived` - Computed Values

```svelte
<script lang="ts">
	let count = $state(0);
	const doubled = $derived(count * 2);

	// For complex derivations
	const filtered = $derived.by(() => {
		return items.filter((item) => item.active);
	});
</script>
```

### `$effect` - Side Effects

```svelte
<script lang="ts">
	let count = $state(0);

	$effect(() => {
		console.log('count changed:', count);
		// Cleanup function (optional)
		return () => console.log('cleaning up');
	});
</script>
```

### `$props` - Component Props

```svelte
<script lang="ts">
	// Basic props
	let { name, age } = $props<{ name: string; age: number }>();

	// With defaults
	let { title = 'Default' } = $props();

	// Bindable props
	let { value = $bindable() } = $props();

	// Rest props
	let { class: className, ...rest } = $props();
</script>
```

## Event Handlers

Use `onclick`, `onchange`, etc. (NO colon):

```svelte
<!-- ✅ Correct -->
<button onclick={() => count++}>Click</button>
<input onchange={(e) => handleChange(e)} />

<!-- ❌ Wrong (Svelte 4 syntax) -->
<button on:click={() => count++}>Click</button>
```

Handle modifiers in JavaScript:

```svelte
<button
	onclick={(e) => {
		e.preventDefault();
		e.stopPropagation();
		handleClick();
	}}>Click</button
>
```

## Snippets (Replace Slots)

```svelte
<!-- Parent.svelte -->
<script>
  import Card from './Card.svelte';
</script>

<Card>
  {#snippet header()}
    <h2>Title</h2>
  {/snippet}

  {#snippet content()}
    <p>Body content</p>
  {/snippet}
</Card>

<!-- Card.svelte -->
<script>
  let { header, content } = $props();
</script>

<div class="card">
  {@render header?.()}
  {@render content?.()}
</div>
```

## Class-Based Stores with Context

```typescript
// stores/auth.svelte.ts
import { createContext } from 'svelte';

class AuthStore {
	user = $state<User | null>(null);
	isAuthenticated = $derived(this.user !== null);

	login(user: User) {
		this.user = user;
	}

	logout() {
		this.user = null;
	}
}

const [getAuth, setAuth] = createContext<AuthStore>();

export const getAuthStore = () => {
	const store = getAuth();
	if (!store) throw new Error('AuthStore not set');
	return store;
};

export const setAuthStore = () => setAuth(new AuthStore());
```

## Anti-Patterns to Avoid

### ❌ Old Reactive Statements

```svelte
<!-- Wrong -->
<script>
	export let count;
	$: doubled = count * 2;
	$: {
		console.log(count);
	}
</script>
```

### ❌ Old Event Syntax

```svelte
<!-- Wrong -->
<button on:click={handler}>Click</button>
<button on:click|preventDefault={handler}>Click</button>
```

### ❌ Old Slot Syntax

```svelte
<!-- Wrong -->
<slot name="header" />
<slot />
```

### ❌ Export Let for Props

```svelte
<!-- Wrong -->
<script>
	export let name;
	export let count = 0;
</script>
```

## When to Use This Skill

- Writing any new `.svelte` component
- Reviewing/updating existing components to Svelte 5
- Migrating from Svelte 4 patterns
- Ensuring consistent syntax across the codebase

## Icon Imports

```typescript
// ✅ Correct
import { Icon } from '@lucide/svelte';

// ❌ Wrong
import Icon from 'lucide-svelte/icons/Icon.svelte';
```
