import { describe, expect, it } from 'vitest';
import type { AcquisitionJob } from '../../types/remoteSource';
import {
	isAcquisitionTerminal,
	statusFromAcquisitionJob,
	uniqueDiagnosticMessage,
	withClearedHandoffJob,
} from './display';

function job(overrides: Partial<AcquisitionJob> = {}): AcquisitionJob {
	return {
		jobId: 'remote-job-1',
		providerId: 'audible',
		status: 'acquiring',
		progress: {
			stage: 'download',
			percentage: 40,
			message: 'Downloading.',
			bytesDownloaded: undefined,
			bytesTotal: undefined,
			currentTitleId: 'B000000001',
			currentItemIndex: 1,
			totalItems: 1,
			terminal: false,
		},
		materializedFiles: [
			{
				inputId: 'provider-input-1',
				titleId: 'B000000001',
				path: '/session/book.m4b',
				sizeBytes: 1024,
				sha256: 'audio-sha',
			},
		],
		supplementalAssets: [],
		diagnostics: [],
		...overrides,
	};
}

describe('remote source acquisition display', () => {
	it('keeps polling while a job is neither terminal nor a finished status', () => {
		expect(isAcquisitionTerminal(job())).toBe(false);
		expect(isAcquisitionTerminal(job({ status: 'failed' }))).toBe(true);
		expect(isAcquisitionTerminal(job({ status: 'cancelled' }))).toBe(true);
		expect(
			isAcquisitionTerminal(
				job({
					status: 'acquiring',
					progress: {
						stage: 'download',
						percentage: 100,
						message: 'Done.',
						bytesDownloaded: undefined,
						bytesTotal: undefined,
						currentTitleId: 'B000000001',
						currentItemIndex: 1,
						totalItems: 1,
						terminal: true,
					},
				}),
			),
		).toBe(true);
	});

	it('does not leak duplicate diagnostic text into the status line', () => {
		const message = uniqueDiagnosticMessage([
			{ kind: 'downloadFailed', titleId: undefined, message: ' Token expired. ' },
			{ kind: 'downloadFailed', titleId: undefined, message: 'Token expired.' },
			{ kind: 'validationFailed', titleId: undefined, message: 'Retrying.' },
		]);
		expect(message).toBe('Token expired. Retrying.');
	});

	it('clears staged handoff files after a failed Input import so retry cannot reuse them', () => {
		const cleaned = withClearedHandoffJob(job());
		expect(cleaned.materializedFiles).toEqual([]);
		expect(cleaned.supplementalAssets).toEqual([]);
		expect(statusFromAcquisitionJob(job())).toBe('Downloading.');
	});
});
