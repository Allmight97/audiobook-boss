import { describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../types/audio';
import type {
	AcquisitionJob,
	RemoteLibraryResponse,
	RemoteRelease,
	RemoteSourceProviderCapabilities,
	RemoteTitle,
} from '../../types/remoteSource';
import type { InputOwner } from '../inputSession';
import { createRemoteSourceOwner, type RemoteSourceOwner } from './owner';
import {
	ORDER_LOCKED_IMPORT_MESSAGE,
	STAGED_FILES_REMOVED_SUFFIX,
	type RemoteSourceWorkflowServices,
} from './workflow';

const primaryPdfFileName = 'Being You - A New Science of Consciousness - Supplemental PDF.pdf';

function createDeferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
}

function fileList(): FileListInfo {
	return {
		files: [
			{
				inputId: 'current-input-1',
				path: '/session/book.m4b',
				size: 1,
				duration: 1,
				isValid: true,
			},
		],
		totalDuration: 1,
		totalSize: 1,
		validCount: 1,
		invalidCount: 0,
		selectedDecoders: [null],
	};
}

function remoteTitle(overrides: Partial<RemoteTitle> = {}): RemoteTitle {
	return {
		providerId: 'audible',
		titleId: 'B000000001',
		title: 'Example Book',
		authors: ['Example Author'],
		narrators: ['Example Narrator'],
		durationSeconds: 3600,
		coverUrl: undefined,
		supplementalPdfAvailable: true,
		acquired: false,
		availability: {
			status: 'available',
			acquirable: true,
			label: 'Available',
			detail: undefined,
		},
		unsupportedReasons: [],
		...overrides,
	};
}

function library(): RemoteLibraryResponse {
	return {
		providerId: 'audible',
		titles: [remoteTitle()],
		diagnostics: [],
	};
}

function runningJob(overrides: Partial<AcquisitionJob> = {}): AcquisitionJob {
	return {
		jobId: 'remote-job-1',
		providerId: 'audible',
		status: 'acquiring',
		progress: {
			stage: 'download',
			percentage: 10,
			message: 'Downloading audiobook.',
			bytesDownloaded: undefined,
			bytesTotal: undefined,
			currentTitleId: 'B000000001',
			currentItemIndex: 1,
			totalItems: 1,
			terminal: false,
		},
		materializedFiles: [],
		supplementalAssets: [],
		diagnostics: [],
		...overrides,
	};
}

function downloadingJob(): AcquisitionJob {
	return runningJob({
		progress: {
			stage: 'download',
			percentage: 40,
			message: 'Downloading audiobook.',
			bytesDownloaded: 78_105_334,
			bytesTotal: 156_210_669,
			currentTitleId: 'B000000001',
			currentItemIndex: 1,
			totalItems: 1,
			terminal: false,
		},
	});
}

function terminalJob(): AcquisitionJob {
	return {
		jobId: 'remote-job-1',
		providerId: 'audible',
		status: 'validated',
		progress: {
			stage: 'importHandoff',
			percentage: 100,
			message: 'Importing acquired audiobook.',
			bytesDownloaded: undefined,
			bytesTotal: undefined,
			currentTitleId: 'B000000001',
			currentItemIndex: 1,
			totalItems: 1,
			terminal: true,
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
		supplementalAssets: [
			{
				assetId: 'pdf-1',
				inputId: 'provider-input-1',
				titleId: 'B000000001',
				path: '/session/book.pdf',
				fileName: primaryPdfFileName,
				sizeBytes: 32,
				sha256: 'pdf-sha',
			},
		],
		diagnostics: [],
	};
}

function providerCapabilities(): RemoteSourceProviderCapabilities[] {
	return [
		{
			providerId: 'audible',
			label: 'Audible',
			authFlow: 'externalBrowserHandoff',
			supportsLibraryScan: true,
			supportsPagedScan: false,
			supportsTypeaheadFilter: true,
			supportsSupplementalPdf: true,
			supportsMaterializedAudio: true,
			supportsReleaseSearch: false,
			supportsReleaseGrab: false,
			supportsRefresh: true,
			requiresLiveSession: true,
			knownUnsupportedReasons: [],
		},
		{
			providerId: 'indexer',
			label: 'Indexer',
			authFlow: 'apiKey',
			supportsLibraryScan: false,
			supportsPagedScan: false,
			supportsTypeaheadFilter: false,
			supportsSupplementalPdf: false,
			supportsMaterializedAudio: false,
			supportsReleaseSearch: true,
			supportsReleaseGrab: true,
			supportsRefresh: false,
			requiresLiveSession: false,
			knownUnsupportedReasons: ['indexerConnectionRequired'],
		},
	];
}

function indexerRelease(overrides: Partial<RemoteRelease> = {}): RemoteRelease {
	return {
		providerId: 'indexer',
		guid: 'release-guid-1',
		indexerId: 7,
		title: 'Example Release',
		indexer: 'Example Indexer',
		sizeBytes: 1_024_000_000,
		protocol: 'torrent',
		seeders: 12,
		...overrides,
	};
}

function makeServices(
	overrides: Partial<RemoteSourceWorkflowServices> = {},
): RemoteSourceWorkflowServices {
	return {
		listProviders: vi.fn(async () => providerCapabilities()),
		getAccountState: vi.fn(async (providerId) => ({
			providerId,
			status: 'connected' as const,
		})),
		startAuth: vi.fn(async (providerId) => ({
			providerId,
			authorizationUrl: 'https://example.test/auth',
			handoffPathHint: '/tmp/handoff',
			message: 'Open Audible to continue.',
		})),
		openAuthorizationUrl: vi.fn(async () => undefined),
		completeAuth: vi.fn(async (providerId) => ({
			providerId,
			status: 'connected' as const,
		})),
		logout: vi.fn(async (providerId) => ({
			providerId,
			status: 'needsAuth' as const,
		})),
		loadLibrary: vi.fn(async () => library()),
		searchReleases: vi.fn(async () => ({
			providerId: 'indexer' as const,
			releases: [indexerRelease()],
			diagnostics: [],
		})),
		grabRelease: vi.fn(async () => ({
			providerId: 'indexer' as const,
			accepted: true,
			message: 'Release sent to Indexer.',
			diagnostics: [],
		})),
		startAcquisition: vi.fn(async (_providerId, _selections) => runningJob()),
		getAcquisitionStatus: vi.fn(async () => terminalJob()),
		cancelAcquisition: vi.fn(async () => ({
			...runningJob(),
			status: 'cancelled' as const,
			progress: {
				...runningJob().progress!,
				terminal: true,
				message: 'Cancelled.',
			},
		})),
		purgeSession: vi.fn(async () => undefined),
		importMaterializedPaths: vi.fn(async () => ({
			status: 'imported' as const,
			fileList: fileList(),
		})),
		sleep: vi.fn(async () => undefined),
		...overrides,
	};
}

function makeOwner(
	services: RemoteSourceWorkflowServices,
	loadCoverArtFromUrl?: (url: string) => Promise<number[]>,
): RemoteSourceOwner {
	return createRemoteSourceOwner({
		services,
		input: {} as InputOwner,
		loadCoverArtFromUrl,
	});
}

describe('remote source acquisition workflow', () => {
	it('rekeys supplemental PDFs through the Input file list after a successful handoff', async () => {
		const services = makeServices();
		const owner = makeOwner(services);
		owner.patch({ selectedTitleIds: new Set(['B000000001']) });

		await owner.runAction({
			type: 'acquireSelected',
		});

		expect(services.startAcquisition).toHaveBeenCalledWith('audible', [
			{ titleId: 'B000000001', includeSupplementalPdf: false },
		]);
		expect(services.importMaterializedPaths).toHaveBeenCalledWith(['/session/book.m4b']);
		expect(services.purgeSession).not.toHaveBeenCalled();
		expect(owner.processingAssets(['current-input-1'])).toEqual({
			'current-input-1': [
				{
					assetId: 'pdf-1',
					inputId: 'current-input-1',
					titleId: 'B000000001',
					path: '/session/book.pdf',
					fileName: primaryPdfFileName,
					sizeBytes: 32,
					sha256: 'pdf-sha',
				},
			],
		});
		expect(owner.view().statusMessage).toBe('1 acquired title imported.');
	});

	it('publishes polled getAcquisitionStatus download progress before the job terminals', async () => {
		const published: Array<{
			readonly stage: string;
			readonly percentage: number;
			readonly message: string;
			readonly bytesDownloaded?: number;
			readonly bytesTotal?: number;
		}> = [];
		let polls = 0;
		let owner!: RemoteSourceOwner;
		const services = makeServices({
			getAcquisitionStatus: vi.fn(async () => {
				polls += 1;
				if (polls === 1) {
					return downloadingJob();
				}
				const progress = owner.view().activeJob?.progress;
				if (progress) {
					published.push({
						stage: progress.stage,
						percentage: progress.percentage,
						message: progress.message,
						bytesDownloaded: progress.bytesDownloaded,
						bytesTotal: progress.bytesTotal,
					});
				}
				return terminalJob();
			}),
		});
		owner = makeOwner(services);
		owner.patch({ selectedTitleIds: new Set(['B000000001']) });
		await owner.runAction({ type: 'acquireSelected' });

		expect(published).toContainEqual({
			stage: 'download',
			percentage: 40,
			message: 'Downloading audiobook.',
			bytesDownloaded: 78_105_334,
			bytesTotal: 156_210_669,
		});
		expect(services.getAcquisitionStatus).toHaveBeenCalled();
	});

	it('purges staged remote files when Input import is blocked by an order lock', async () => {
		const services = makeServices({
			importMaterializedPaths: vi.fn(async () => ({
				status: 'blocked' as const,
				message: ORDER_LOCKED_IMPORT_MESSAGE,
			})),
		});
		const owner = makeOwner(services);
		owner.patch({ selectedTitleIds: new Set(['B000000001']) });

		await owner.runAction({
			type: 'acquireSelected',
		});

		expect(services.purgeSession).toHaveBeenCalledWith('remote-job-1');
		expect(owner.processingAssets(['current-input-1'])).toBeUndefined();
		expect(owner.view().activeJob?.materializedFiles).toEqual([]);
		expect(owner.view().statusMessage).toContain(STAGED_FILES_REMOVED_SUFFIX);
	});

	it('does not call native cancel when the dialog closes during an in-flight acquisition', async () => {
		let polls = 0;
		let owner!: RemoteSourceOwner;
		const services = makeServices({
			getAcquisitionStatus: vi.fn(async () => {
				polls += 1;
				if (polls === 1) {
					owner.close();
					return runningJob();
				}
				return terminalJob();
			}),
		});
		owner = makeOwner(services);
		owner.patch({
			isOpen: true,
			selectedTitleIds: new Set(['B000000001']),
		});

		await owner.runAction({
			type: 'acquireSelected',
		});

		expect(services.cancelAcquisition).not.toHaveBeenCalled();
		expect(owner.view().isOpen).toBe(false);
		expect(services.importMaterializedPaths).toHaveBeenCalled();
	});

	it('cancels only through the explicit cancel action', async () => {
		const services = makeServices();
		const owner = makeOwner(services);
		owner.patch({ activeJob: runningJob() });

		await owner.runAction({
			type: 'cancelActiveAcquisition',
		});

		expect(services.cancelAcquisition).toHaveBeenCalledWith('remote-job-1');
		expect(owner.view().statusMessage).toBe('Cancelled.');
	});

	it('does not let a late acquisition poll overwrite native cancellation', async () => {
		const latePoll = createDeferred<AcquisitionJob>();
		const services = makeServices({
			getAcquisitionStatus: vi.fn(() => latePoll.promise),
		});
		const owner = makeOwner(services);
		owner.patch({ selectedTitleIds: new Set(['B000000001']) });

		const acquisition = owner.runAction({
			type: 'acquireSelected',
		});
		await vi.waitFor(() => expect(services.getAcquisitionStatus).toHaveBeenCalledTimes(1));

		await owner.runAction({
			type: 'cancelActiveAcquisition',
		});
		expect(owner.view().activeJob?.status).toBe('cancelled');
		expect(owner.view().isBusy).toBe(false);

		latePoll.resolve(downloadingJob());
		await acquisition;

		expect(owner.view().activeJob?.status).toBe('cancelled');
		expect(owner.view().activeJob?.progress?.percentage).toBe(10);
		expect(services.importMaterializedPaths).not.toHaveBeenCalled();
	});

	it('keeps acquisition state and supplemental assets isolated across owners', async () => {
		const first = makeOwner(makeServices());
		const second = makeOwner(makeServices());
		first.patch({ selectedTitleIds: new Set(['B000000001']) });
		second.patch({ selectedTitleIds: new Set(['B000000001']), isOpen: true });

		await first.runAction({ type: 'acquireSelected' });
		await second.runAction({ type: 'acquireSelected' });
		expect(first.hasCompanions('current-input-1')).toBe(true);
		expect(second.hasCompanions('current-input-1')).toBe(true);

		first.reset();
		expect(first.hasCompanions('current-input-1')).toBe(false);
		expect(second.hasCompanions('current-input-1')).toBe(true);
		expect(second.view().isOpen).toBe(true);
	});

	it('keeps retain, reconcile, and purge sequencing isolated across owners', async () => {
		const firstServices = makeServices();
		const secondServices = makeServices();
		const first = makeOwner(firstServices);
		const second = makeOwner(secondServices);
		first.patch({ selectedTitleIds: new Set(['B000000001']) });
		second.patch({ selectedTitleIds: new Set(['B000000001']) });
		await first.runAction({ type: 'acquireSelected' });
		await second.runAction({ type: 'acquireSelected' });
		await first.reconcileWithInput(fileList().files);
		await second.reconcileWithInput(fileList().files);

		await first.withSubmissionRetention(['current-input-1'], async () => 'accepted');
		await first.reconcileWithInput([]);
		await second.reconcileWithInput([]);

		expect(firstServices.purgeSession).not.toHaveBeenCalled();
		expect(secondServices.purgeSession).toHaveBeenCalledWith('remote-job-1');
		await first.settleTerminalWork({
			inputIds: ['current-input-1'],
			completedInputIds: [],
		});
		expect(firstServices.purgeSession).toHaveBeenCalledWith('remote-job-1');
	});

	it('keeps cover preview cancellation and cache state isolated across owners', async () => {
		const firstLoad = createDeferred<number[]>();
		const first = makeOwner(makeServices(), () => firstLoad.promise);
		const second = makeOwner(makeServices(), async () => [0xff, 0xd8, 0xff]);

		first.scheduleCoverPreviews(['https://covers.example/first.jpg']);
		second.scheduleCoverPreviews(['https://covers.example/second.jpg']);
		await vi.waitFor(() =>
			expect(second.coverPreview('https://covers.example/second.jpg').status).toBe('ready'),
		);

		first.reset();
		firstLoad.resolve([0xff, 0xd8, 0xff]);
		await Promise.resolve();
		expect(first.coverPreview('https://covers.example/first.jpg').status).toBe('idle');
		expect(second.coverPreview('https://covers.example/second.jpg').status).toBe('ready');
	});
});
