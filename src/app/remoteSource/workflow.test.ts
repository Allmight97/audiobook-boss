import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../types/audio';
import type { AcquisitionJob, RemoteLibraryResponse, RemoteTitle } from '../../types/remoteSource';
import { supplementalAssetsForInputIds } from './sessionAssets';
import { patchRemoteSourceState, resetRemoteSourceState } from './state';
import {
	makeRemoteSourceWorkflowServicesLayer,
	ORDER_LOCKED_IMPORT_MESSAGE,
	runRemoteSourceWorkflow,
	STAGED_FILES_REMOVED_SUFFIX,
	type RemoteSourceWorkflowServices,
} from './workflow';
import { resetRemoteSourceSessionAssets } from './sessionAssets';
import { remoteSourceState } from './state';

const primaryPdfFileName = 'Being You - A New Science of Consciousness - Supplemental PDF.pdf';

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

function runningJob(): AcquisitionJob {
	return {
		jobId: 'remote-job-1',
		providerId: 'audible',
		status: 'acquiring',
		progress: {
			stage: 'download',
			percentage: 10,
			message: 'Downloading.',
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
	};
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

function makeServices(
	overrides: Partial<RemoteSourceWorkflowServices> = {},
): RemoteSourceWorkflowServices {
	return {
		getAccountState: vi.fn(async () => ({
			providerId: 'audible' as const,
			status: 'connected' as const,
		})),
		startAuth: vi.fn(async () => ({
			providerId: 'audible' as const,
			authorizationUrl: 'https://example.test/auth',
			handoffPathHint: '/tmp/handoff',
			message: 'Open Audible to continue.',
		})),
		openAuthorizationUrl: vi.fn(async () => undefined),
		completeAuth: vi.fn(async () => ({
			providerId: 'audible' as const,
			status: 'connected' as const,
		})),
		logout: vi.fn(async () => ({
			providerId: 'audible' as const,
			status: 'needsAuth' as const,
		})),
		loadLibrary: vi.fn(async () => library()),
		startAcquisition: vi.fn(async () => runningJob()),
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

describe('remote source acquisition workflow', () => {
	beforeEach(() => {
		resetRemoteSourceState();
		resetRemoteSourceSessionAssets();
	});

	it('rekeys supplemental PDFs through the Input file list after a successful handoff', async () => {
		const services = makeServices();
		patchRemoteSourceState({ selectedTitleIds: new Set(['B000000001']) });

		await runRemoteSourceWorkflow(makeRemoteSourceWorkflowServicesLayer(services), {
			type: 'acquireSelected',
		});

		expect(services.startAcquisition).toHaveBeenCalledWith([
			{ titleId: 'B000000001', includeSupplementalPdf: false },
		]);
		expect(services.importMaterializedPaths).toHaveBeenCalledWith(['/session/book.m4b']);
		expect(services.purgeSession).not.toHaveBeenCalled();
		expect(supplementalAssetsForInputIds(['current-input-1'])).toEqual({
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
		expect(remoteSourceState.statusMessage).toBe('1 acquired title imported.');
	});

	it('purges staged remote files when Input import is blocked by an order lock', async () => {
		const services = makeServices({
			importMaterializedPaths: vi.fn(async () => ({
				status: 'blocked' as const,
				message: ORDER_LOCKED_IMPORT_MESSAGE,
			})),
		});
		patchRemoteSourceState({ selectedTitleIds: new Set(['B000000001']) });

		await runRemoteSourceWorkflow(makeRemoteSourceWorkflowServicesLayer(services), {
			type: 'acquireSelected',
		});

		expect(services.purgeSession).toHaveBeenCalledWith('remote-job-1');
		expect(supplementalAssetsForInputIds(['current-input-1'])).toBeUndefined();
		expect(remoteSourceState.activeJob?.materializedFiles).toEqual([]);
		expect(remoteSourceState.statusMessage).toContain(STAGED_FILES_REMOVED_SUFFIX);
	});

	it('does not call native cancel when the dialog closes during an in-flight acquisition', async () => {
		let polls = 0;
		const services = makeServices({
			getAcquisitionStatus: vi.fn(async () => {
				polls += 1;
				if (polls === 1) {
					patchRemoteSourceState({ isOpen: false, didHydrateOpenDialog: false });
					return runningJob();
				}
				return terminalJob();
			}),
		});
		patchRemoteSourceState({
			isOpen: true,
			selectedTitleIds: new Set(['B000000001']),
		});

		await runRemoteSourceWorkflow(makeRemoteSourceWorkflowServicesLayer(services), {
			type: 'acquireSelected',
		});

		expect(services.cancelAcquisition).not.toHaveBeenCalled();
		expect(remoteSourceState.isOpen).toBe(false);
		expect(services.importMaterializedPaths).toHaveBeenCalled();
	});

	it('cancels only through the explicit cancel action', async () => {
		const services = makeServices();
		patchRemoteSourceState({ activeJob: runningJob() });

		await runRemoteSourceWorkflow(makeRemoteSourceWorkflowServicesLayer(services), {
			type: 'cancelActiveAcquisition',
		});

		expect(services.cancelAcquisition).toHaveBeenCalledWith('remote-job-1');
		expect(remoteSourceState.statusMessage).toBe('Cancelled.');
	});
});
