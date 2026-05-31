# SvelteKit Remote Functions Reference

Official docs: <https://svelte.dev/docs/kit/remote-functions>

## Choosing an API

- `query`: dynamic server reads. Cached by serialized argument and deduped per request/client reactive context.
- `query.batch`: read API for avoiding n+1 calls; receives an array of validated args and returns a resolver `(arg, index) => value`.
- `query.live`: real-time reads; handler returns an `AsyncIterable`, usually an async generator.
- `form`: progressively enhanced form submissions. Supports no-JS fallback, field helpers, validation issues, redirects, and `enhance`.
- `command`: non-form mutations called from JS. Return serializable results; do not redirect from commands.
- `prerender`: static/read-mostly data computed at build time. Use `inputs` for known args and `dynamic: true` only when non-prerendered args must still work server-side.

## Client Query Patterns

```svelte
<script lang="ts">
	import { getThing, updateThing } from './thing.remote';

	let { id }: { id: string } = $props();
	const thing = $derived(await getThing(id));

	async function save() {
		const result = await updateThing({ id, name: thing.name });
		// Show success/error via the app's toast pattern.
	}
</script>
```

Create query instances in a reactive context (`$derived`, `$effect`, or markup) before reading `current`, `loading`, `error`, or awaiting them. For one-off event-handler reads that should bypass the reactive cache, use `await getThing(id).run()`.

## Forms

```typescript
import { form, getRequestEvent } from '$app/server';
import { redirect } from '@sveltejs/kit';
import { z } from 'zod';
import { requireAuth } from '$lib/utils/auth/guards.server';
import { USER_ROLES } from '$lib/utils/auth/roles';

export const createThing = form(
	z.object({
		name: z.string().min(1),
		_secret: z.string().min(8)
	}),
	async ({ name }) => {
		const event = getRequestEvent();
		requireAuth(event, { requiredRole: USER_ROLES.USER });

		const id = await insertThing({ name });
		redirect(303, `/things/${id}`);
	}
);
```

```svelte
<form {...createThing}>
	<input {...createThing.fields.name.as('text')} />
	<input {...createThing.fields._secret.as('password')} />
	<button disabled={createThing.pending}>Save</button>
</form>
```

- Add `enctype="multipart/form-data"` when using file fields.
- Use `createThing.enhance(async (form) => { ... await form.submit() ... })` for custom toast/reset behavior.
- Returned form data is available as `createThing.result` and is ephemeral.
- For checkbox values, make the schema optional/defaulted or use explicit coercion because unchecked boxes are absent from `FormData`.
- For radio and checkbox groups, pass the submitted value as the second argument to `.as(...)`.

## Cache And Mutations

- After a successful mutation, refresh every query whose cached data may now be stale.
- On the server, `await getThing(id).refresh()` attaches fresh data to the mutation response and avoids an extra client round trip.
- For unknown client-visible query instances, use `requested(getThing, limit)` inside a `command` or `form`.

```typescript
import { command, requested } from '$app/server';

export const bulkUpdate = command(schema, async (input) => {
	// mutate rows...
	await requested(getThing, 20).refreshAll();
	return { success: true };
});
```

- Use client-side `.updates(...)` / `.withOverride(...)` only for optimistic UI where rollback/error behavior is acceptable and explicitly handled.
- For `query.live`, call `.reconnect()` on affected live query resources from mutations when the stream should restart.

## Advanced Reads

```typescript
import { prerender, query } from '$app/server';
import { z } from 'zod';

export const getWeather = query.batch(z.string(), async (cityIds) => {
	const rows = await loadWeatherRows(cityIds);
	const byCity = new Map(rows.map((row) => [row.cityId, row]));
	return (cityId) => byCity.get(cityId) ?? null;
});

export const getStaticFlags = prerender(async () => loadFeatureFlags());

export const getStaticPost = prerender(z.string(), async (slug) => loadStaticPost(slug), {
	inputs: () => ['first-post', 'second-post'],
	dynamic: false
});
```

Use `query.batch` when many child components call the same read with different ids during one render. Use `prerender` only for data that changes at most once per deployment. When the entire page has `export const prerender = true`, do not use dynamic `query` calls on that page.

## Validation And Errors

- Validation schemas protect generated endpoints from stale clients and direct bad requests.
- If a schema changes incompatibly, expect validation failures from users on older deployments.
- SvelteKit can centralize remote validation failure responses with the `handleValidationError` server hook.
- Avoid `'unchecked'` schemas unless there is a deliberate, documented reason; the endpoint is still public to direct requests.
- Prefer schema coercion only where browser form semantics require it, such as number and checkbox inputs.
- `getRequestEvent()` should be called before `await` when targeting environments without `AsyncLocalStorage`.
- Inside remote functions, `route`, `params`, and `url` describe the page that called the remote function. Never authorize by inspecting those values.

## Review Checklist

- [ ] File is named `.remote.ts` and is not inside `src/lib/server`.
- [ ] Every handler validates input and calls `requireAuth` with the correct role.
- [ ] Fallible work uses `Result`/`ResultAsync` or an explicitly justified alternative.
- [ ] Queries are reads only; commands/forms perform mutations.
- [ ] Mutations refresh or update all stale query resources.
- [ ] Components create query instances in reactive contexts or use `.run()` for one-off reads.
- [ ] Forms handle pending/result/error UX and sensitive fields safely.
- [ ] Internal navigation in surrounding Svelte code uses `resolve(...)` from `$app/paths`.
