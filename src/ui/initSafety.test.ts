import { describe, expect, it, vi } from 'vitest';
import { runInitSteps } from './initSafety';

describe('runInitSteps', () => {
	it('throws immediately when a step throws', () => {
		const executed: string[] = [];
		const error = new Error('boom');
		const spy = vi.spyOn(console, 'error').mockImplementation(() => {});

		expect(() =>
			runInitSteps([
				{ label: 'first', init: () => executed.push('first') },
				{
					label: 'broken',
					init: () => {
						executed.push('broken');
						throw error;
					},
				},
				{ label: 'last', init: () => executed.push('last') },
			]),
		).toThrow('[ui:init] broken failed');

		expect(executed).toEqual(['first', 'broken']);
		expect(spy).toHaveBeenCalledTimes(1);
		expect(spy.mock.calls[0]?.[0]).toContain('[ui:init] broken failed');

		spy.mockRestore();
	});
});
