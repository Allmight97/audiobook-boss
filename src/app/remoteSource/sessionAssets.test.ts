import { beforeEach, describe, expect, it, vi, type Mock } from 'vitest';
import type { AcquisitionJob } from '../../types/remoteSource';
import type { FileListInfo } from '../../types/audio';
import { createRemoteSourceSessionAssets, type RemoteSourceSessionAssets } from './sessionAssets';

const purgeRemoteSourceSessionMock = vi.fn();
const primaryPdfFileName = 'Being You - A New Science of Consciousness - Supplemental PDF.pdf';
const secondaryPdfFileName = 'Secure Love - Supplemental PDF.pdf';

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
	let assets: RemoteSourceSessionAssets;
	let onChange: Mock<() => void>;

	beforeEach(() => {
		purgeRemoteSourceSessionMock.mockReset();
		purgeRemoteSourceSessionMock.mockResolvedValue(undefined);
		onChange = vi.fn();
		assets = createRemoteSourceSessionAssets({
			purgeSession: purgeRemoteSourceSessionMock,
			onChange,
		});
	});

	it('publishes when supplemental assets are registered', () => {
		assets.register(acquisitionJob(), fileList());
		expect(onChange).toHaveBeenCalledTimes(1);
	});

	it('rekeys provider supplemental assets to the imported file input id', () => {
		assets.register(acquisitionJob(), fileList());
		expect(assets.processingAssets(['current-input-1'])).toEqual({
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

	it('summarizes single-file companion assets without exposing paths', () => {
		assets.register(acquisitionJob(), fileList());
		const summary = assets.companionSummary(['current-input-1']);

		expect(summary).toEqual({
			text: primaryPdfFileName,
			title: primaryPdfFileName,
			pdfCount: 1,
			fileCountWithCompanions: 1,
		});
		expect(summary.title).not.toContain('/session/');
		expect(assets.hasCompanions('current-input-1')).toBe(true);
	});

	it('summarizes multi-file companion assets by selected count', () => {
		assets.register(acquisitionJob(), fileList());
		const summary = assets.companionSummary(['current-input-1', 'current-input-2']);

		expect(summary.text).toBe('1 PDF across 2 selected files');
		expect(summary.title).toBe('1 PDF across 2 selected files');
		expect(summary.pdfCount).toBe(1);
		expect(summary.fileCountWithCompanions).toBe(1);
	});

	it('keeps multi-file companion summaries count-based for long PDF names', () => {
		assets.register(multiTitleAcquisitionJob(), multiTitleFileList());
		const summary = assets.companionSummary(['current-input-1', 'current-input-2']);

		expect(summary.text).toBe('2 PDFs across 2 selected files');
		expect(summary.title).toBe('2 PDFs across 2 selected files');
		expect(summary.text).not.toContain(primaryPdfFileName);
		expect(summary.text).not.toContain(secondaryPdfFileName);
		expect(summary.pdfCount).toBe(2);
		expect(summary.fileCountWithCompanions).toBe(2);
	});

	it('purges acquired session roots when inputs leave the session', async () => {
		assets.register(acquisitionJob(), fileList());
		await assets.reconcileInput(fileList().files);
		await assets.reconcileInput([]);
		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
		expect(assets.processingAssets(['current-input-1'])).toBeUndefined();
	});

	it('defers input cleanup while an accepted work operation retains the input', async () => {
		assets.register(acquisitionJob(), fileList());
		await assets.reconcileInput(fileList().files);
		await assets.withSubmissionRetention(['current-input-1'], async () => 'accepted');
		await assets.reconcileInput([]);
		expect(purgeRemoteSourceSessionMock).not.toHaveBeenCalled();
		expect(assets.processingAssets(['current-input-1'])).toBeDefined();
		await assets.settleTerminalWork({
			inputIds: ['current-input-1'],
			completedInputIds: [],
		});
		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
		expect(assets.processingAssets(['current-input-1'])).toBeUndefined();
	});

	it('does not purge a shared acquired session while a retained sibling remains in flight', async () => {
		assets.register(multiTitleAcquisitionJob(), multiTitleFileList());
		await assets.reconcileInput(multiTitleFileList().files);
		await assets.withSubmissionRetention(['current-input-1'], async () => 'accepted');
		await assets.withSubmissionRetention(['current-input-2'], async () => 'accepted');
		await assets.reconcileInput([]);
		await assets.settleTerminalWork({
			inputIds: ['current-input-1'],
			completedInputIds: [],
		});
		expect(purgeRemoteSourceSessionMock).not.toHaveBeenCalled();
		expect(assets.processingAssets(['current-input-2'])).toBeDefined();
		await assets.settleTerminalWork({
			inputIds: ['current-input-2'],
			completedInputIds: [],
		});
		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
		expect(assets.processingAssets(['current-input-2'])).toBeUndefined();
	});

	it('waits to purge shared acquisition sessions until every registered input is removable', async () => {
		assets.register(multiTitleAcquisitionJob(), multiTitleFileList());
		await assets.reconcileInput(multiTitleFileList().files);
		await assets.reconcileInput([multiTitleFileList().files[1]!]);
		expect(purgeRemoteSourceSessionMock).not.toHaveBeenCalled();
		expect(assets.processingAssets(['current-input-1'])).toBeUndefined();
		expect(assets.processingAssets(['current-input-2'])).toEqual({
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
		await assets.reconcileInput([]);
		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledTimes(1);
		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
		expect(assets.processingAssets(['current-input-2'])).toBeUndefined();
	});

	it('preserves the submission error after releasing pending cleanup', async () => {
		assets.register(acquisitionJob(), fileList());
		await assets.reconcileInput(fileList().files);
		const submissionError = new Error('submission failed');
		const submission = assets.withSubmissionRetention(['current-input-1'], async () => {
			await assets.reconcileInput([]);
			throw submissionError;
		});
		await expect(submission).rejects.toBe(submissionError);
		expect(purgeRemoteSourceSessionMock).toHaveBeenCalledWith('remote-job-1');
	});

	it('keeps cleanup failure non-blocking and forgets the stale association', async () => {
		purgeRemoteSourceSessionMock.mockRejectedValueOnce(new Error('busy'));
		assets.register(acquisitionJob(), fileList());
		await assets.reconcileInput(fileList().files);
		await expect(assets.reconcileInput([])).resolves.toBeUndefined();
		expect(assets.processingAssets(['current-input-1'])).toBeUndefined();
	});

	it('does not share assets or purge state across instances', () => {
		const other = createRemoteSourceSessionAssets({
			purgeSession: vi.fn(async () => undefined),
			onChange: vi.fn(),
		});
		assets.register(acquisitionJob(), fileList());
		expect(assets.hasCompanions('current-input-1')).toBe(true);
		expect(other.hasCompanions('current-input-1')).toBe(false);
	});
});
