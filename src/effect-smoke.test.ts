import { Effect } from 'effect';
import { describe, expect, it } from 'vitest';

describe('Effect dependency smoke', () => {
	it('imports and runs Effect in the current frontend test runtime', async () => {
		const result = await Effect.runPromise(
			Effect.gen(function* () {
				const base = yield* Effect.succeed(21);
				return base * 2;
			}),
		);

		expect(result).toBe(42);
	});
});
