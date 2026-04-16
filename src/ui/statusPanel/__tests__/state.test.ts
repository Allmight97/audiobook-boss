import { describe, expect, it } from 'vitest';
import { buildStatus, isActiveEventStage, type ProcessingStatus } from '../state';

describe('statusPanel state helpers', () => {
	it('forces idle status to percentage 0 and drops active-only fields', () => {
		const status = buildStatus('idle', 47, 'Ready to process audiobook', {
			currentFile: '/books/alpha.m4b',
			etaSeconds: 12,
		});

		expect(status).toEqual({
			stage: 'idle',
			percentage: 0,
			message: 'Ready to process audiobook',
		});
		expect(status).not.toHaveProperty('currentFile');
		expect(status).not.toHaveProperty('etaSeconds');
	});

	it.each([
		['analyzing', true],
		['converting', true],
		['writing', true],
		['completed', false],
		['failed', false],
		['cancelled', false],
		['idle', false],
	] as const)('isActiveEventStage(%s) -> %s', (stage, expected) => {
		expect(isActiveEventStage(stage as ProcessingStatus['stage'])).toBe(expected);
	});
});
