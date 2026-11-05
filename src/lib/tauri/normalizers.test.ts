import { describe, expect, it } from 'vitest';

import { normalizeSessionUpdate } from './normalizers';
import { recordedFresh } from './recordedFresh';

describe('session update normalization', () => {
	it('normalizes cached library title optionals and preserves meaningful empty account/job snapshots', async () => {
		const seed = recordedFresh.session.remote!;
		const remote = {
			...seed,
			account: null,
			indexer: { ...seed.indexer, releases: [] },
		};
		const update = normalizeSessionUpdate({
			revision: 3,
			titles: null,
			selection: null,
			metadata: null,
			lookup: null,
			audio: null,
			output: null,
			remote,
			remoteAcquisition: { revision: 2, acquiring: false, job: null },
			remoteLibrary: {
				revision: 2,
				titles: [
					{
						providerId: 'audible',
						titleId: 'book',
						title: 'Book',
						authors: [],
						narrators: [],
						durationSeconds: null,
						coverUrl: null,
						supplementalPdfAvailable: false,
						acquired: false,
						availability: {
							status: 'available',
							acquirable: true,
							label: 'Available',
							detail: null,
						},
						unsupportedReasons: [],
					},
				],
				diagnostics: [{ kind: 'validationFailed', titleId: null, message: 'Partial library' }],
			},
		});
		expect(update.remote?.account).toBeNull();
		expect(update.remoteAcquisition?.job).toBeNull();
		expect(update.remoteLibrary?.titles[0]).not.toHaveProperty('coverUrl');
		expect(update.remoteLibrary?.titles[0].availability).not.toHaveProperty('detail');
		expect(update.remoteLibrary?.diagnostics[0]).not.toHaveProperty('titleId');
		const progressOnly = normalizeSessionUpdate({
			revision: 4,
			titles: null,
			selection: null,
			metadata: null,
			lookup: null,
			audio: null,
			output: null,
			remote: null,
			remoteLibrary: null,
			remoteAcquisition: {
				revision: 3,
				acquiring: false,
				job: {
					jobId: 'job',
					providerId: 'audible',
					status: 'acquiring',
					progress: {
						stage: 'download',
						percentage: 40,
						message: 'Downloading audiobook.',
						terminal: false,
						bytesDownloaded: null,
						bytesTotal: null,
						currentTitleId: null,
						currentItemIndex: null,
						totalItems: null,
					},
					materializedFiles: [],
					supplementalAssets: [],
					diagnostics: [],
					handoff: null,
					terminal: false,
					settled: false,
				},
			},
		});
		expect(progressOnly.remote).toBeUndefined();
		expect(progressOnly.remoteLibrary).toBeUndefined();
		expect(progressOnly.remoteAcquisition?.job?.progress).not.toHaveProperty('bytesTotal');
	});
});
