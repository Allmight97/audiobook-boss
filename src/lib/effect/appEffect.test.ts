import { describe, expect, it } from 'vitest';
import {
	Effect,
	makeWorkflowLayer,
	makeWorkflowServiceTag,
	runAppEffect,
	tryAppPromise,
} from './appEffect';
import type { UnexpectedWorkflowError } from './appEffect';

describe('AppEffect kernel', () => {
	it('runs Svelte-callable promise bridges with provided workflow services', async () => {
		const NumberService = makeWorkflowServiceTag<'test/NumberService', { value: number }>(
			'test/NumberService',
		);
		const program = Effect.gen(function* () {
			const service = yield* NumberService;
			return service.value * 2;
		});

		const result = await runAppEffect(
			program.pipe(Effect.provide(makeWorkflowLayer(NumberService, { value: 21 }))),
		);

		expect(result).toBe(42);
	});

	it('maps rejected promises into typed workflow errors', async () => {
		const error = await runAppEffect(
			tryAppPromise(
				() => Promise.reject(new Error('dependency failed')),
				'Workflow dependency failed.',
			).pipe(Effect.flip),
		);

		expect(error).toMatchObject({
			_tag: 'UnexpectedWorkflowError',
			message: 'Workflow dependency failed.',
			cause: expect.any(Error),
		} satisfies Partial<UnexpectedWorkflowError>);
	});
});
