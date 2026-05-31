---
name: sveltekit-remote-functions
description: Guides SvelteKit Remote Function design and implementation for query, query.batch, query.live, form, command, prerender, cache refresh, validation, and auth. Use when creating, editing, reviewing, or debugging `.remote.ts` files or component code that calls remote functions.
license: MIT
compatibility: opencode
metadata:
  audience: fullstack-devs
  enforce: backend-patterns
---

# SvelteKit Remote Functions

Remote functions are SvelteKit's type-safe client/server boundary. They are exported from `.remote.ts` or `.remote.js` files, can be called from app code, and always execute on the server.

Current docs checked: <https://svelte.dev/docs/kit/remote-functions>. Remote functions are still experimental; verify official docs when changing advanced behavior.

## Quick Start

```typescript
import { command, getRequestEvent, query } from '$app/server';
import { error } from '@sveltejs/kit';
import { ResultAsync } from 'neverthrow';
import { z } from 'zod';
import { toError } from '$lib/server/utils/errors';
import { requireAuth } from '$lib/utils/auth/guards.server';
import { USER_ROLES } from '$lib/utils/auth/roles';

const idSchema = z.string().min(1);
const updateSchema = z.object({ id: idSchema, name: z.string().min(1) });

export const getThing = query(idSchema, async (id) => {
	const event = getRequestEvent();
	requireAuth(event, { requiredRole: USER_ROLES.USER });

	const result = await ResultAsync.fromPromise(loadThing(id), (cause) =>
		toError(cause, 'Failed to load thing')
	);

	return result.match(
		(thing) => {
			if (!thing) error(404, 'Thing not found');
			return thing;
		},
		(cause) => {
			console.error('Failed to load thing:', cause);
			error(503, 'Unable to load thing');
		}
	);
});

export const updateThing = command(updateSchema, async (input) => {
	const event = getRequestEvent();
	requireAuth(event, { requiredRole: USER_ROLES.USER });

	const updateResult = await ResultAsync.fromPromise(updateThingRow(input), (cause) =>
		toError(cause, 'Failed to update thing')
	);

	if (updateResult.isErr()) {
		console.error('Failed to update thing:', updateResult.error);
		return { success: false, message: 'Failed to update thing' };
	}

	await getThing(input.id).refresh();
	return { success: true };
});
```

## Workflow

1. Confirm `kit.experimental.remoteFunctions: true`; for component `await`, confirm `compilerOptions.experimental.async: true`.
2. Put remote functions in `.remote.ts` files anywhere under `src` except `src/lib/server`.
3. Choose the right API: `query` for reads, `query.batch` for n+1 reads, `query.live` for real-time reads, `form` for progressive form mutations, `command` for JS mutations, `prerender` for build-time data.
4. Validate every argument with Standard Schema, usually `zod` in this repo.
5. Call `requireAuth(getRequestEvent(), { requiredRole: USER_ROLES.* })` inside every handler.
6. Use `Result`/`ResultAsync` for fallible database, network, parsing, and cache-refresh work when touching `.remote.ts`.
7. Refresh or update all stale query resources after successful mutations.
8. In component callers, create query instances in reactive contexts (`$derived`, `$effect`, or markup); use `.run()` for one-off event-handler reads.

## Rules

- Use `error(status, message)` for true request failures; return typed `{ success, message }` objects for expected mutation outcomes.
- Do not redirect from `command`; redirects are valid in `form`, `query`, and `prerender`.
- Do not catch `error(...)` or `redirect(...)` unless you rethrow them.
- Prefix sensitive form field names with `_` so invalid no-JS submissions do not repopulate secrets.
- Never authorize by inspecting remote `route`, `params`, or `url`; they describe the caller page, not the generated endpoint.
- Route internal navigation in surrounding Svelte code through `resolve(...)` from `$app/paths`.

## Advanced Reference

See [REFERENCE.md](REFERENCE.md) for forms, `enhance`, `requested(...)`, optimistic updates, `query.batch`, `query.live`, `prerender`, validation failures, and review checklists.
