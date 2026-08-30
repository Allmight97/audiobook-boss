import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AcquisitionJob } from '../../types/remoteSource';
import type { FileListInfo } from '../../types/audio';

const purgeRemoteSourceSessionMock = vi.hoisted(() => vi.fn());
const primaryPdfFileName = 'Being You - A New Science of Consciousness - Supplemental PDF.pdf';
const secondaryPdfFileName = 'Secure Love - Supplemental PDF.pdf';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		purgeRemoteSourceSession: purgeRemoteSourceSessionMock,
	},
}));

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

function multiTitleFileList(): FileListInfo {
	const base = fileList();
	return {
		...base,
		files: [
			...base.files,
			{
				inputId: 'current-input-2',
				path: '/session/book-two.m4b',
				size: 1,
				duration: 1,
				isValid: true,
			},
		],
		totalDuration: 2,
		totalSize: 2,
		validCount: 2,
		selectedDecoders: [null, null],
	};
}

function acquisitionJob(): AcquisitionJob {
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

function multiTitleAcquisitionJob(): AcquisitionJob {
	const base = acquisitionJob();
	return {
		...base,
		materializedFiles: [
			...base.materializedFiles,
			{
				inputId: 'provider-input-2',
				titleId: 'B000000002',
				path: '/session/book-two.m4b',
				sizeBytes: 2048,
				sha256: 'audio-sha-two',
			},
		],
		supplementalAssets: [
			...base.supplementalAssets,
			{
				assetId: 'pdf-2',
				inputId: 'provider-input-2',
				titleId: 'B000000002',
				path: '/session/book-two.pdf',
				fileName: secondaryPdfFileName,
				sizeBytes: 64,
				sha256: 'pdf-sha-two',
			},
		],
	};
}

describe('remote source session assets', () => {
	beforeEach(async () => {
		purgeRemoteSourceSessionMock.mockReset();
		purgeRemoteSourceSessionMock.mockResolvedValue(undefined);
		const { resetRemoteSourceSessionAssets } = await import('./sessionAssets');
		resetRemoteSourceSessionAssets();
	});

	it('notifies subscribers when supplemental assets are registered', async () => {
		const module = await import('./sessionAssets');
		const listener = vi.fn();
		const unsubscribe = module.subscribeRemoteSourceSupplementalAssets(listener);

		module.registerRemoteSourceSupplementalAssets(acquisitionJob(), fileList());

		expect(listener).toHaveBeenCalledTimes(1);
		unsubscribe();
	});

	it('rekeys provider supplemental assets to the imported file input id', async () => {
		const module = await import('./sessionAssets');

		module.registerRemoteSourceSupplementalAssets(acquisitionJob(), fileList());

		expect(module.supplementalAssetsForInputIds(['current-input-1'])).toEqual({
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
	});

	it('summarizes single-file companion assets without exposing paths', async () => {
		const module = await import('./sessionAssets');
		module.registerRemoteSourceSupplementalAssets(acquisitionJob(), fileList());

		const summary = module.companionSummaryForInputIds(['current-input-1']);

		expect(summary).toEqual({
			text: primaryPdfFileName,
			title: primaryPdfFileName,
			pdfCount: 1,
			fileCountWithCompanions: 1,
		});
		expect(summary.title).not.toContain('/session/');
		expect(module.hasSupplementalAssetsForInputId('current-input-1')).toBe(true);
	});

	it('summarizes multi-file companion assets by selected count', async () => {
		const module = await import('./sessionAssets');
		module.registerRemoteSourceSupplementalAssets(acquisitionJob(), fileList());

		const summary = module.companionSummaryForInputIds(['current-input-1', 'current-input-2']);

		expect(summary.text).toBe('1 PDF across 2 selected files');
		expect(summary.title).toBe('1 PDF across 2 selected files');
		expect(summary.pdfCount).toBe(1);
		expect(summary.fileCountWithCompanions).toBe(1);
	});

	it('keeps multi-file companion summaries count-based when every file has a long PDF name', async () => {
		const module = await import('./sessionAssets');
		module.registerRemoteSourceSupplementalAssets(multiTitleAcquisitionJob(), multiTitleFileList());

		const summary = module.companionSummaryForInputIds(['current-input-1', 'current-input-2']);

		expect(summary.text).toBe('2 PDFs across 2 selected files');
		expect(summary.title).toBe('2 PDFs across 2 selected files');
		expect(summary.text).not.toContain(primaryPdfFileName);
		expect(summary.text).not.toContain(secondaryPdfFileName);
		expect(summary.pdfCount).toBe(2);
		expect(summary.fileCountWithCompanions).toBe(2);
	});

	it('purges acquired session roots for explicit input ids', async () => {
		const module = await import('./sessionAssets');
		module.registerRemoteSourceSupplementalAssets(acquisitionJob(), fileList());

		await module.purgeRemoteSourceSessionsForInputIds(['current-input-1']);

		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
		expect(module.supplementalAssetsForInputIds(['current-input-1'])).toBeUndefined();
	});

	it('defers draft cleanup purge while an accepted work operation retains the input', async () => {
		const module = await import('./sessionAssets');
		module.registerRemoteSourceSupplementalAssets(acquisitionJob(), fileList());
		module.retainRemoteSourceSessionsForInputIds(['current-input-1']);

		await module.purgeRemoteSourceSessionsForInputIds(['current-input-1']);

		expect(purgeRemoteSourceSessionMock).not.toHaveBeenCalled();
		expect(module.supplementalAssetsForInputIds(['current-input-1'])).toBeDefined();

		const pendingPurgeInputIds = module.releaseRemoteSourceSessionRetainers(['current-input-1']);
		await module.purgeRemoteSourceSessionsForInputIds(pendingPurgeInputIds);

		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
		expect(module.supplementalAssetsForInputIds(['current-input-1'])).toBeUndefined();
	});

	it('does not purge a shared acquired session while a retained sibling remains in flight', async () => {
		const module = await import('./sessionAssets');
		module.registerRemoteSourceSupplementalAssets(multiTitleAcquisitionJob(), multiTitleFileList());
		module.retainRemoteSourceSessionsForInputIds(['current-input-1', 'current-input-2']);

		await module.purgeRemoteSourceSessionsForInputIds(['current-input-1', 'current-input-2']);
		const firstPendingPurge = module.releaseRemoteSourceSessionRetainers(['current-input-1']);
		await module.purgeRemoteSourceSessionsForInputIds(firstPendingPurge);

		expect(purgeRemoteSourceSessionMock).not.toHaveBeenCalled();
		expect(module.supplementalAssetsForInputIds(['current-input-1'])).toBeUndefined();
		expect(module.supplementalAssetsForInputIds(['current-input-2'])).toBeDefined();

		const secondPendingPurge = module.releaseRemoteSourceSessionRetainers(['current-input-2']);
		await module.purgeRemoteSourceSessionsForInputIds(secondPendingPurge);

		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
		expect(module.supplementalAssetsForInputIds(['current-input-2'])).toBeUndefined();
	});

	it('waits to purge shared acquisition sessions until every registered input is removable', async () => {
		const module = await import('./sessionAssets');
		module.registerRemoteSourceSupplementalAssets(multiTitleAcquisitionJob(), multiTitleFileList());

		await module.purgeRemoteSourceSessionsForInputIds(['current-input-1']);

		expect(purgeRemoteSourceSessionMock).not.toHaveBeenCalled();
		expect(module.supplementalAssetsForInputIds(['current-input-1'])).toBeUndefined();
		expect(module.supplementalAssetsForInputIds(['current-input-2'])).toEqual({
			'current-input-2': [
				{
					assetId: 'pdf-2',
					inputId: 'current-input-2',
					titleId: 'B000000002',
					path: '/session/book-two.pdf',
					fileName: secondaryPdfFileName,
					sizeBytes: 64,
					sha256: 'pdf-sha-two',
				},
			],
		});

		await module.purgeRemoteSourceSessionsForInputIds(['current-input-2']);

		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledTimes(1);
		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
		expect(module.supplementalAssetsForInputIds(['current-input-2'])).toBeUndefined();
	});

	it('purges remote sessions for input ids that leave the Input session', async () => {
		const module = await import('./sessionAssets');
		module.registerRemoteSourceSupplementalAssets(acquisitionJob(), fileList());
		await module.reconcileRemoteSourceSessionsWithInput(fileList().files);
		await module.reconcileRemoteSourceSessionsWithInput([]);

		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
		expect(module.supplementalAssetsForInputIds(['current-input-1'])).toBeUndefined();
	});
});
