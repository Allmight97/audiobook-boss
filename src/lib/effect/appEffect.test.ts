import { describe, expect, it } from 'vitest';
import { Data } from 'effect';
import {
	Effect,
	makeWorkflowLayer,
	makeWorkflowServiceTag,
	runAppEffect,
	workflowTryPromise,
	workflowTrySync,
} from './appEffect';

class HarnessWorkflowFailed extends Data.TaggedError('HarnessWorkflowFailed')<{
	readonly message: string;
	readonly cause: unknown;
}> {}

function harnessFailure(message: string, cause: unknown): HarnessWorkflowFailed {
	return new HarnessWorkflowFailed({ message, cause });
}

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

	it('maps rejected promises into owner-specific workflow errors via workflowTryPromise', async () => {
		const error = await runAppEffect(
			workflowTryPromise(
				() => Promise.reject(new Error('owner dependency failed')),
				'Owner workflow dependency failed.',
				harnessFailure,
			).pipe(Effect.flip),
		);

		expect(error).toMatchObject({
			_tag: 'HarnessWorkflowFailed',
			message: 'Owner workflow dependency failed.',
			cause: expect.any(Error),
		} satisfies Partial<HarnessWorkflowFailed>);
	});

	it('maps synchronous throws into owner-specific workflow errors via workflowTrySync', async () => {
		const error = await runAppEffect(
			workflowTrySync(
				() => {
					throw new Error('sync dependency failed');
				},
				'Owner sync workflow failed.',
				harnessFailure,
			).pipe(Effect.flip),
		);

		expect(error).toMatchObject({
			_tag: 'HarnessWorkflowFailed',
			message: 'Owner sync workflow failed.',
			cause: expect.any(Error),
		} satisfies Partial<HarnessWorkflowFailed>);
	});
});
