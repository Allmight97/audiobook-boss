import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AcquisitionJob } from '../../types/remoteSource';
import type { FileListInfo, ProcessCommandResult } from '../../types/audio';

const purgeRemoteSourceSessionMock = vi.hoisted(() => vi.fn());

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
				fileName: 'Supplemental PDF.pdf',
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
				fileName: 'Supplemental PDF 2.pdf',
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
		const module = await import('./sessionAssets.svelte');
		module.removeRemoteSourceSupplementalAssets(['current-input-1', 'current-input-2']);
	});

	it('rekeys provider supplemental assets to the imported file input id', async () => {
		const module = await import('./sessionAssets.svelte');

		module.registerRemoteSourceSupplementalAssets(acquisitionJob(), fileList());

		expect(module.supplementalAssetsForInputIds(['current-input-1'])).toEqual({
			'current-input-1': [
				{
					assetId: 'pdf-1',
					inputId: 'current-input-1',
					titleId: 'B000000001',
					path: '/session/book.pdf',
					fileName: 'Supplemental PDF.pdf',
					sizeBytes: 32,
					sha256: 'pdf-sha',
				},
			],
		});
	});

	it('purges acquired session roots after matching final batch success', async () => {
		const module = await import('./sessionAssets.svelte');
		module.registerRemoteSourceSupplementalAssets(acquisitionJob(), fileList());
		const result: ProcessCommandResult = {
			jobType: 'batch',
			summary: { total: 1, succeeded: 1, skipped: 0, cancelled: 0, failed: 0 },
			results: [
				{
					inputIndex: 0,
					status: 'success',
					message: 'ok',
					error: undefined,
					previewFilePath: undefined,
					previewActualSeconds: undefined,
					jobId: 'processing-job-1',
				},
			],
		};

		await module.purgeSuccessfulRemoteSourceSessions(result, ['current-input-1']);

		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
		expect(module.supplementalAssetsForInputIds(['current-input-1'])).toBeUndefined();
	});

	it('waits to purge shared acquisition sessions until every registered input is removable', async () => {
		const module = await import('./sessionAssets.svelte');
		module.registerRemoteSourceSupplementalAssets(multiTitleAcquisitionJob(), multiTitleFileList());
		const partialResult: ProcessCommandResult = {
			jobType: 'batch',
			summary: { total: 2, succeeded: 1, skipped: 0, cancelled: 0, failed: 1 },
			results: [
				{
					inputIndex: 0,
					status: 'success',
					message: 'ok',
					error: undefined,
					previewFilePath: undefined,
					previewActualSeconds: undefined,
					jobId: 'processing-job-1',
				},
				{
					inputIndex: 1,
					status: 'failed',
					message: 'failed',
					error: {
						code: 'processing_failed',
						category: 'processing',
						message: 'failed',
					},
					previewFilePath: undefined,
					previewActualSeconds: undefined,
					jobId: 'processing-job-2',
				},
			],
		};

		await module.purgeSuccessfulRemoteSourceSessions(partialResult, [
			'current-input-1',
			'current-input-2',
		]);

		expect(purgeRemoteSourceSessionMock).not.toHaveBeenCalled();
		expect(module.supplementalAssetsForInputIds(['current-input-1'])).toBeUndefined();
		expect(module.supplementalAssetsForInputIds(['current-input-2'])).toEqual({
			'current-input-2': [
				{
					assetId: 'pdf-2',
					inputId: 'current-input-2',
					titleId: 'B000000002',
					path: '/session/book-two.pdf',
					fileName: 'Supplemental PDF 2.pdf',
					sizeBytes: 64,
					sha256: 'pdf-sha-two',
				},
			],
		});

		const retryResult: ProcessCommandResult = {
			jobType: 'batch',
			summary: { total: 1, succeeded: 1, skipped: 0, cancelled: 0, failed: 0 },
			results: [
				{
					inputIndex: 0,
					status: 'success',
					message: 'ok',
					error: undefined,
					previewFilePath: undefined,
					previewActualSeconds: undefined,
					jobId: 'processing-job-3',
				},
			],
		};

		await module.purgeSuccessfulRemoteSourceSessions(retryResult, ['current-input-2']);

		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledTimes(1);
		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
		expect(module.supplementalAssetsForInputIds(['current-input-2'])).toBeUndefined();
	});
});
