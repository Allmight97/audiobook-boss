import { describe, expect, it, vi } from 'vitest';
import { Effect, runAppEffect } from '../../../lib/effect/appEffect';
import type { AudioFile, FileListInfo, SupportedAudioImportMetadata } from '../../../types/audio';
import type { FileListAppendResult } from '../../fileList/appendResult';
import {
	importAnalysisWorkflowExecution,
	makeImportAnalysisWorkflowServicesLayer,
	type ImportAnalysisWorkflowServices,
} from '../importAnalysisWorkflow';

function audioFile(path: string): AudioFile {
	return {
		path,
		isValid: true,
		duration: 1,
		size: 100,
		format: 'm4b',
	} as AudioFile;
}

function fileListInfo(files: AudioFile[] = [audioFile('/books/new.m4b')]): FileListInfo {
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.length,
		totalSize: files.length * 100,
		validCount: files.length,
		invalidCount: 0,
	} as FileListInfo;
}

function appendResult(
	outcome: FileListAppendResult['outcome'] = 'append',
	files: AudioFile[] = [audioFile('/books/new.m4b')],
): FileListAppendResult {
	if (outcome === 'duplicateOnly') {
		return {
			outcome,
			fileList: null,
			incomingFiles: files,
			appendedFiles: [],
			existingFiles: [],
		};
	}
	return {
		outcome,
		fileList: fileListInfo(files),
		incomingFiles: files,
		appendedFiles: files,
		existingFiles: [],
	};
}

const supportedAudioMetadata: SupportedAudioImportMetadata = {
	formats: [
		{ extension: 'mp3', label: 'MP3' },
		{ extension: 'm4a', label: 'M4A/M4B' },
		{ extension: 'm4b', label: 'M4A/M4B' },
		{ extension: 'aac', label: 'AAC' },
		{ extension: 'wav', label: 'WAV' },
		{ extension: 'flac', label: 'FLAC' },
	],
	extensions: ['mp3', 'm4a', 'm4b', 'aac', 'wav', 'flac'],
	formatsText: 'MP3, M4A/M4B, AAC, WAV, and FLAC',
	supportText: 'Supports MP3, M4A/M4B, AAC, WAV, and FLAC audio files',
};

function makeHarness(overrides: Partial<ImportAnalysisWorkflowServices> = {}) {
	const services: ImportAnalysisWorkflowServices = {
		isOrderLocked: vi.fn(() => false),
		getSupportedAudioImportMetadata: vi.fn(async () => supportedAudioMetadata),
		openFiles: vi.fn(async () => ['/books/new.m4b']),
		openDirectory: vi.fn(async () => '/books/series'),
		discoverAudioImportPaths: vi.fn(async (paths: string[]) => paths),
		analyzeAudioFiles: vi.fn(async () => fileListInfo()),
		persistPendingMetadataDraftsForCurrentSelection: vi.fn(async () => true),
		appendFileList: vi.fn(() => appendResult()),
		pushStatusPanelTransientStatus: vi.fn(),
		setFileImportError: vi.fn(),
		clearFileImportError: vi.fn(),
		console: { error: vi.fn() },
		...overrides,
	};

	return {
		services,
		layer: makeImportAnalysisWorkflowServicesLayer(services),
	};
}

describe('ImportAnalysisWorkflow', () => {
	it('blocks import while file order is locked', async () => {
		const harness = makeHarness({ isOrderLocked: vi.fn(() => true) });

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'clickToSelect',
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.openFiles).not.toHaveBeenCalled();
		expect(harness.services.discoverAudioImportPaths).not.toHaveBeenCalled();
		expect(harness.services.setFileImportError).toHaveBeenCalledWith(
			'Order locked while processing. Wait for completion to add files.',
		);
	});

	it('does nothing when file picker is cancelled', async () => {
		const harness = makeHarness({ openFiles: vi.fn(async () => null) });

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'clickToSelect',
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.analyzeAudioFiles).not.toHaveBeenCalled();
		expect(harness.services.appendFileList).not.toHaveBeenCalled();
	});

	it('reports unsupported imports after backend discovery returns no audio files', async () => {
		const harness = makeHarness({
			discoverAudioImportPaths: vi.fn(async () => []),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'importPaths',
				paths: ['/tmp/notes.txt', '/tmp/image.png'],
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.discoverAudioImportPaths).toHaveBeenCalledWith([
			'/tmp/notes.txt',
			'/tmp/image.png',
		]);
		expect(harness.services.analyzeAudioFiles).not.toHaveBeenCalled();
		expect(harness.services.setFileImportError).toHaveBeenCalledWith(
			'No supported audio files found. Please use MP3, M4A/M4B, AAC, WAV, and FLAC files.',
		);
	});

	it('uses backend-discovered import paths before analysis', async () => {
		const harness = makeHarness({
			discoverAudioImportPaths: vi.fn(async () => ['/tmp/a.wav', '/tmp/c.M4B']),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'importPaths',
				paths: ['/tmp/a.wav', '/tmp/b.txt', '/tmp/c.M4B'],
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.discoverAudioImportPaths).toHaveBeenCalledWith([
			'/tmp/a.wav',
			'/tmp/b.txt',
			'/tmp/c.M4B',
		]);
		expect(harness.services.analyzeAudioFiles).toHaveBeenCalledWith(['/tmp/a.wav', '/tmp/c.M4B']);
	});

	it('opens a folder and recursively discovers audio paths before analysis', async () => {
		const harness = makeHarness({
			openDirectory: vi.fn(async () => '/books/author'),
			discoverAudioImportPaths: vi.fn(async () => ['/books/author/Book 1.m4b']),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'clickToSelectFolder',
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.openDirectory).toHaveBeenCalledTimes(1);
		expect(harness.services.discoverAudioImportPaths).toHaveBeenCalledWith(['/books/author']);
		expect(harness.services.analyzeAudioFiles).toHaveBeenCalledWith(['/books/author/Book 1.m4b']);
	});

	it('reports analysis failure without appending files', async () => {
		const cause = new Error('analysis failed');
		const harness = makeHarness({
			analyzeAudioFiles: vi.fn(async () => {
				throw cause;
			}),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'importPaths',
				paths: ['/tmp/a.m4b'],
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.appendFileList).not.toHaveBeenCalled();
		expect(harness.services.console.error).toHaveBeenCalledWith('Failed to analyze files:', cause);
		expect(harness.services.setFileImportError).toHaveBeenCalledWith(
			'Failed to analyze files. Please try again.',
		);
	});

	it('reports file picker failures without rendering technical details', async () => {
		const cause = new Error('dialog failed');
		const harness = makeHarness({
			openFiles: vi.fn(async () => {
				throw cause;
			}),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'clickToSelect',
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.analyzeAudioFiles).not.toHaveBeenCalled();
		expect(harness.services.console.error).toHaveBeenCalledWith(
			'Failed to open file dialog:',
			cause,
		);
		expect(harness.services.setFileImportError).toHaveBeenCalledWith(
			'Failed to open file dialog. Please try again.',
		);
	});

	it('does not append files when metadata staging fails', async () => {
		const harness = makeHarness({
			persistPendingMetadataDraftsForCurrentSelection: vi.fn(async () => false),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'clickToSelect',
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.appendFileList).not.toHaveBeenCalled();
		expect(harness.services.setFileImportError).toHaveBeenCalledWith(
			'Fix metadata validation errors before adding files.',
		);
	});

	it('reports metadata staging exceptions separately from analysis failures', async () => {
		const cause = new Error('draft store failed');
		const harness = makeHarness({
			persistPendingMetadataDraftsForCurrentSelection: vi.fn(async () => {
				throw cause;
			}),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'clickToSelect',
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.appendFileList).not.toHaveBeenCalled();
		expect(harness.services.console.error).toHaveBeenCalledWith(
			'Failed to stage metadata drafts:',
			cause,
		);
		expect(harness.services.setFileImportError).toHaveBeenCalledWith(
			'Failed to prepare metadata drafts before adding files. Please try again.',
		);
	});

	it('appends analyzed files and clears import errors on success', async () => {
		const existingFiles = [audioFile('/books/existing.m4b')];
		const analyzed = fileListInfo([audioFile('/books/new.m4b')]);
		const harness = makeHarness({
			analyzeAudioFiles: vi.fn(async () => analyzed),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'clickToSelect',
				existingFiles,
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.appendFileList).toHaveBeenCalledWith(analyzed, {
			existingFiles,
			showDuplicateStatus: false,
		});
		expect(harness.services.clearFileImportError).toHaveBeenCalledTimes(1);
	});

	it('reports duplicate-only imports from the file list append result', async () => {
		const existingFiles = [audioFile('/books/existing.m4b')];
		const analyzed = fileListInfo([audioFile('/books/existing.m4b')]);
		const harness = makeHarness({
			analyzeAudioFiles: vi.fn(async () => analyzed),
			appendFileList: vi.fn(() => appendResult('duplicateOnly', analyzed.files)),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'importPaths',
				paths: ['/books/existing.m4b'],
				existingFiles,
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.appendFileList).toHaveBeenCalledWith(analyzed, {
			existingFiles,
			showDuplicateStatus: false,
		});
		expect(harness.services.pushStatusPanelTransientStatus).toHaveBeenCalledWith(
			'No new files added. All analyzed files were already in the list.',
			{ ttlMs: 2000 },
		);
		expect(harness.services.setFileImportError).toHaveBeenCalledWith(
			'No new files added. All analyzed files were already in the list.',
		);
		expect(harness.services.clearFileImportError).not.toHaveBeenCalled();
	});
});
