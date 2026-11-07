import { describe, expect, it } from 'vitest';
import { statusOf, type StatusInputs } from '../status';
import { processingFailure } from '../submit';
import type { OutputSnapshot } from '../../../lib/generated/tauri';

const concurrency = { selection: 'auto', effective: 4 };

function inputs(over: Partial<StatusInputs>): StatusInputs {
	return { previewRun: null, submission: null, localError: null, concurrency, ...over };
}

function failedPreview(): NonNullable<OutputSnapshot['previewRun']> {
	const progress = {
		stage: 'failed' as const,
		percentage: 100,
		message: 'No room',
		etaSeconds: null,
	};
	return {
		openReady: false,
		cover: null,
		operation: {
			operationId: 'preview-1',
			sequence: 0,
			revision: 1,
			kind: 'processingBatch',
			status: 'failed',
			phase: 'finished',
			title: 'Book',
			startedAtMs: 1,
			finishedAtMs: 2,
			cancellable: false,
			cancelRequested: false,
			progress,
			logTail: [],
			children: [],
		},
	};
}

describe('statusOf', () => {
	it('is Idle with the concurrency line when nothing has happened', () => {
		expect(statusOf(inputs({}))).toMatchObject({
			statusText: 'Idle',
			stepText: 'Current Step: Ready',
			stepTone: 'normal',
			concurrencyText: 'Max jobs: 4 (Auto)',
		});
	});

	it('shows a refusal as an error line on Idle', () => {
		const view = statusOf(
			inputs({ submission: { kind: 'refused', reason: { kind: 'noOutputDirectory' } } }),
		);
		expect(view).toMatchObject({ statusText: 'Idle', stepTone: 'error' });
		expect(view.stepText).toContain('Choose an output folder');
	});

	it('keeps the finished preview status text above a blocked submission error', () => {
		const view = statusOf(
			inputs({
				previewRun: failedPreview(),
				submission: { kind: 'blocked', message: 'No room' },
			}),
		);
		expect(view).toMatchObject({
			statusText: 'Failed',
			stepText: 'Error: No room',
			stepTone: 'error',
		});
	});

	it('shows a local error over the engine submission line', () => {
		const view = statusOf(
			inputs({
				submission: { kind: 'submitted', operationId: 'op', title: 'A', leftOut: 0 },
				localError: 'Preview could not be opened: gone',
			}),
		);
		expect(view.stepText).toBe('Error: Preview could not be opened: gone');
	});

	it('words a failure once, with or without a message', () => {
		expect(processingFailure({ code: 'io', category: 'io', message: 'Disk full' })).toBe(
			'Processing failed: Disk full',
		);
		expect(processingFailure({ code: 'io', category: 'io', message: '' })).toBe(
			'Processing failed.',
		);
	});
});
